# Choosing the HTML parser

ferrolink needs three things from a page: the URL-bearing attributes of a few
elements, every `id` (and `<a name>`), and the first `<base href>`. It does not
need a DOM. Every backend therefore drives the same collector
(`src/scan/mod.rs`); the backends differ only in how they tokenize.

Four candidates were measured on the same workload:

| Backend | Approach | License |
| --- | --- | --- |
| [html5gum](https://crates.io/crates/html5gum) 0.8 | WHATWG tokenizer, borrowed callback events, naive raw-text state switching | MIT |
| [html5ever](https://crates.io/crates/html5ever) 0.40 | Servo's WHATWG tokenizer with a ferrolink token sink that makes the tree builder's state switches; no tree construction | MIT OR Apache-2.0 |
| [lol_html](https://crates.io/crates/lol_html) 3.0 | Cloudflare's streaming rewriter with a CSS selector for the relevant elements | BSD-3-Clause, pulls in MPL-2.0 crates |
| [Ferroni](https://crates.io/crates/ferroni) 2.5 | Regular expressions over the raw text: one pattern for comments and start tags, a second to split attributes; raw-text elements skipped with `memmem` | BSD-2-Clause |

Only the Ferroni backend ships. The tokenizers, the `parser-comparison`
feature, `--parser`, and `examples/compare_parsers.rs` were removed rather
than kept in agreement with a decision already made; the last tree with all
four backends is commit `cb42b5b`.

## Workload

The Ardo documentation Pages artifact of 2026-10-22: 335 pages, 10.4 MB of
HTML, 1,043 files, about 22,600 internal and 2,400 external references. Apple
M1 Ultra (16 performance and 4 efficiency cores), macOS, Rust 1.99, release
build. The first round of this comparison ran on the 318-page build of
2026-10-13 (9.2 MB); a number below that comes from that build says so.

Reproduce at commit `cb42b5b`, the last with every backend:

```bash
cargo run --release --features parser-comparison --example compare_parsers -- <build-dir>
cargo build --release --features parser-comparison
hyperfine -N --warmup 5 'target/release/ferrolink check <build-dir> --parser ferroni'
```

On the current tree, `ferrolink check <build-dir> --timings` prints the scan
phase and hyperfine times the whole check.

## Results

### Scan throughput, one thread

Median of 15 passes over all pages, after a warm-up pass. "Differing pages"
compares links (URL and origin) and anchors with html5ever's. The Ferroni row
uses the regex backend as tuned below, on Ferroni 2.5.2.

| Backend | ms | MB/s | Pages differing |
| --- | ---: | ---: | ---: |
| Ferroni | 50.6 | 206 | 0 |
| html5gum | 61.1 | 170 | 0 |
| lol_html | 73.3 | 142 | 0 |
| html5ever | 129.0 | 81 | 0 |

All four extract identical links and anchors from all 335 pages, and the full
check reports the same 234 issues with each of them.

With Ferroni 2.3.0 the regex backend and html5gum were level on this build
(60.4 and 61.0 ms). Ferroni 2.4.0 specialized the negated byte-class runs
(`[^"]*+`, `[^<]++`) both patterns are built from
([sebastian-software/ferroni#441](https://github.com/sebastian-software/ferroni/pull/441),
[#443](https://github.com/sebastian-software/ferroni/pull/443)). Ferroni's own
paired measurement of the two corpus steps found about 4% less time; in
ferrolink's backend, where the patterns run over whole documents through the
byte APIs, it brought the scan to 54.8 ms. Taking the capture groups out of
the attribute pattern as well (below) brought it to the 50.6 ms above.

### End to end, all cores

`ferrolink check` on the whole build, walk and checks included (hyperfine, 40
runs, 20 threads unless noted):

| Backend | Wall time |
| --- | ---: |
| html5gum | 23.7 ms ± 0.6 |
| Ferroni | 24.8 ms ± 0.8 |
| lol_html | 30.1 ms ± 1.2 |
| html5ever | 35.4 ms ± 5.6 |
| Ferroni, one thread | 79.7 ms ± 1.7 |
| html5gum, one thread | 91.6 ms ± 1.0 |

On 20 threads the scan phase takes 7.5 ms with Ferroni and 7.3 ms with
html5gum (`--timings`, medians of 15 interleaved runs), the walk 9 ms and the
checks 2 ms, so the regex backend's single-thread lead is not visible end to
end. Compiling the two patterns costs about 0.4 ms per process (the 4-page
fixture site on one thread: 3.9 ms against 3.5 ms). Before the Ferroni 2.2.4
fixes described below, the same check took 33.1 ms ± 1.5 on the 318-page
build.

### Malformed markup

On a page with an unterminated attribute quote, the three tokenizers agree with
a browser: everything after the quote is part of the attribute value. The
regex backend resynchronizes at the next `<` and reports links and `id`s that a
browser never sees. The unit tests pin the same difference for an apostrophe
inside an unquoted attribute value. On comment and raw-text ends (`<!-->`,
`--!>`, `</script foo>`, `<script-x>`) all four agree.

## Tuning the regex backend

The first regex backend took 108 ms on one thread, on the 318-page build. The
patterns, not Ferroni, were most of the gap to html5gum. The markup scan alone,
isolated from ferrolink (one thread, medians of 7 runs):

| Pattern | ms | MB/s |
| --- | ---: | ---: |
| Comments, raw-text elements with content, start and end tags, captures | 66.0 | 139 |
| Without end tags (nothing needs them; every match is an iteration step) | 53.5 | 172 |
| Possessive runs (`[^<]++`, `[^-]++`) instead of lazy `[\s\S]*?` bodies | 48.3 | 190 |
| Without the raw-text alternative tried at every `<` (`memmem` finds the end tag) | 41.7 | 220 |
| Attribute loop unrolled, `normal* (special normal*)*` | 37.8 | 243 |
| No capture groups: name and attributes sliced from the match, `find_iter` | 28.8 | 319 |
| For comparison: the `regex` crate's lazy DFA, same pattern | 25.7 | 357 |

With byte APIs instead of `&str` (no UTF-8 validation of the document and of
every match) and a stricter `id` prefilter, the whole backend went from 108 ms
to 50.3 ms. About 70% of a pass is now the backtracking matcher itself.

The attribute pattern kept four capture groups, for the name and the
double-quoted, single-quoted, or bare value, until Ferroni 2.5.2. Slicing name
and value from a group-free match instead, with `find_iter` as for the markup
pattern, took the whole backend from 55.5 ms to 50.6 ms on the 335-page build
(187 to 206 MB/s) in alternating runs where html5gum stayed at 61 ms. The
last capture path in ferrolink went with it.

## Ferroni findings

Dogfooding Ferroni on this workload turned up three problems in Ferroni:

1. **`captures_iter` did not scale past four threads.** Every `Captures` cloned
   the regex's `Arc` for `Captures::name`, so each match wrote one shared
   reference count. Fixed in #417: patterns without named groups no longer
   touch it. At 20 threads, 60.9 ms became 7.3 ms.
2. **The seek gate of `captures()` wrote shared atomics on every search.**
   Fixed in #417: it writes only on a change and samples its probes per
   thread.
3. **`find_at` consulted the seek on every call.** With dense matches, the
   lazy DFA's scan to the next match came on top of the matcher's, and a
   `find_bytes_at` loop took 49.4 ms where `find_iter_bytes` took 28.8 ms.
   [sebastian-software/ferroni#418](https://github.com/sebastian-software/ferroni/pull/418)
   puts `find*` behind the same gate as the capture methods: 31.6 ms.

Two smaller costs were looked at next.
[sebastian-software/ferroni#420](https://github.com/sebastian-software/ferroni/pull/420)
makes `Match::as_str` slice a `&str` search's text instead of validating every
match (−18% for `find_iter` with `as_str`, −7% for `&caps[i]`). Merging the
thread-local caches a single search touches measured no gain (−0.6% even on
1.6 million tiny `captures_at` calls) and was left out. ferrolink uses the
byte APIs either way.

Ferroni tracks this workload since
[sebastian-software/ferroni#422](https://github.com/sebastian-software/ferroni/pull/422):
both patterns are cases of its source-backed regex corpus (`html_scanning`,
with these fixtures and Ardo pages), and its `regression_bench` runs them on
1, 4 and 8 threads, which catches a shared write per search like the ones
above.

A SIMD fast path for negated class runs (`[^"]*` with `memchr`) was tried and
dropped: 71% of the attribute values on this build are shorter than 16 bytes,
and all value bytes together cost about 2 ms of the 66 ms scan in the byte
loop, so it measured no gain.

## Decision

Ferroni is the default and the only backend in the default build, since
Ferroni 2.5.2. Until then html5gum was the default: with Ferroni 2.2.4 the two
were level, and the tie went to the complete WHATWG tokenizer. With Ferroni
2.4.0 the regex backend is the fastest on one thread, and it is the engine
the family dogfoods, so ferrolink now exercises Ferroni on every build it
checks. The price is known and small for generator output: the regex backend
resynchronizes at the next `<` on malformed markup where a browser would not,
and decodes only the character references ferrolink's table knows
(`decode_char_refs`: numeric references and the eleven named ones that occur
in URLs). On both Ardo builds every backend reports the same issues.

The tokenizers stayed behind a `parser-comparison` feature for a while and
were then removed: three more backends to keep in agreement bought nothing
once the decision was made, and commit `cb42b5b` keeps the comparison
repeatable. What they offered:

- **html5gum** is a complete WHATWG tokenizer with full character reference
  decoding that follows a browser on malformed markup; a fifth slower on one
  thread, level on all cores.
- **lol_html** returns attribute values undecoded and brings four MPL-2.0
  crates through Servo's selector stack.
- **html5ever** is the reference for correctness but half as fast without a
  tree builder, and slower still with one.
