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
| [Ferroni](https://crates.io/crates/ferroni) 2.2 | Regular expressions over the raw text: comments, raw-text elements, start and end tags; attributes split by a second pattern | BSD-2-Clause |

## Workload

The Ardo documentation Pages artifact: 318 pages, 9.2 MB of HTML, 1,087 files,
about 19,400 internal and 1,700 external references. Apple M1 Ultra (16
performance and 4 efficiency cores), macOS, Rust 1.99, release build.

Reproduce with:

```bash
cargo run --release --features parser-comparison --example compare_parsers -- <build-dir>
cargo build --release --features parser-comparison
hyperfine -N --warmup 5 'target/release/ferrolink check <build-dir> --parser html5gum'
```

## Results

### Scan throughput, one thread

Median of 15 passes over all pages, after a warm-up pass. "Differing pages"
compares links (URL and origin) and anchors with html5ever's. The Ferroni row
uses the regex backend as tuned below, on Ferroni 2.2.4, which contains the
fixes described below.

| Backend | ms | MB/s | Pages differing |
| --- | ---: | ---: | ---: |
| Ferroni | 50.3 | 183 | 0 |
| html5gum | 51.1 | 180 | 0 |
| lol_html | 60.9 | 151 | 0 |
| html5ever | 113.0 | 81 | 0 |

All four extract identical links and anchors from all 318 pages, and the full
check reports the same 235 issues with each of them.

### End to end, all cores

`ferrolink check` on the whole build, walk and checks included (hyperfine, 40
runs, 20 threads unless noted):

| Backend | Wall time |
| --- | ---: |
| html5gum | 23.2 ms ± 0.7 |
| Ferroni 2.2.4 | 23.6 ms ± 0.7 |
| lol_html | 28.7 ms ± 2.6 |
| Ferroni 2.2.3 (before the fixes) | 33.1 ms ± 1.5 |
| html5ever | 33.8 ms ± 1.0 |
| html5gum, one thread | 79.0 ms ± 1.4 |

### Malformed markup

On a page with an unterminated attribute quote, the three tokenizers agree with
a browser: everything after the quote is part of the attribute value. The
regex backend resynchronizes at the next `<` and reports links and `id`s that a
browser never sees. The unit tests pin the same difference for an apostrophe
inside an unquoted attribute value. On comment and raw-text ends (`<!-->`,
`--!>`, `</script foo>`, `<script-x>`) all four agree.

## Tuning the regex backend

The first regex backend took 108 ms on one thread. The patterns, not Ferroni,
were most of the gap to html5gum. The markup scan alone, isolated from
ferrolink (one thread, medians of 7 runs):

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

html5gum stays the default and the only backend in the default build. With
Ferroni 2.2.4, the regex backend matches its speed on this build, so the choice rests on
correctness: html5gum is a complete WHATWG tokenizer with full character
reference decoding, and follows a browser on malformed markup, which no regex
approach does. The other backends stay behind the `parser-comparison`
feature so the comparison can be repeated on other builds:

- **Ferroni** matches html5gum on well-formed markup at the same speed (with
  Ferroni 2.2.4), but resynchronizes differently on malformed markup and decodes only
  the character references ferrolink's small table knows.
- **lol_html** comes close in speed but returns attribute values undecoded and
  brings four MPL-2.0 crates through Servo's selector stack.
- **html5ever** is the reference for correctness but half as fast without a
  tree builder, and slower still with one.
