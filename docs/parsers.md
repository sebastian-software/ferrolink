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

Median of 9 passes over all pages, after a warm-up pass. "Differing pages"
compares links (URL and origin) and anchors with html5ever's.

| Backend | ms | MB/s | Pages differing |
| --- | ---: | ---: | ---: |
| html5gum | 51.5 | 178 | 0 |
| lol_html | 60.3 | 152 | 0 |
| html5ever | 111.9 | 82 | 0 |
| Ferroni | 108.0 | 85 | 0 |

All four extract identical links and anchors from all 318 pages, and the full
check reports the same 235 issues with each of them.

### End to end, all cores

`ferrolink check` on the whole build, walk and checks included (hyperfine, 40
runs, 20 threads unless noted):

| Backend | Wall time | Memory |
| --- | ---: | ---: |
| html5gum | 23.3 ms ± 0.9 | 12.9 MiB |
| lol_html | 28.7 ms ± 2.6 | 14.9 MiB |
| Ferroni with the fix below | 29.5 ms ± 1.5 | 14.5 MiB |
| html5ever | 33.8 ms ± 1.0 | 16.0 MiB |
| Ferroni 2.2.3 | 86.7 ms ± 4.8 | 14.8 MiB |
| html5gum, one thread | 79.0 ms ± 1.4 | 7.6 MiB |

### Malformed markup

On a page with an unterminated attribute quote, the three tokenizers agree with
a browser: everything after the quote is part of the attribute value. The
regex backend resynchronizes at the next `<` and reports links and `id`s that a
browser never sees. The unit tests pin the same difference for an apostrophe
inside an unquoted attribute value.

## Ferroni: why the regex backend did not scale

Isolated from ferrolink, Ferroni's markup pattern runs at 139 MB/s with
captures on one thread, 2.5 times the `regex` crate's 54 MB/s on an equivalent
pattern. The single-thread gap to html5gum came from ferrolink's glue in the
regex backend (a second pattern per tag, lowercasing), not from Ferroni.

The multi-threaded result had a different cause. `find_iter` scaled linearly,
but `captures_iter` stopped at about four threads and got slower beyond:

| Threads | `find_iter` | `captures_iter` (2.2.3) |
| ---: | ---: | ---: |
| 1 | 55.9 ms | 66.0 ms |
| 4 | 18.1 ms | 36.1 ms |
| 8 | 7.5 ms | 50.4 ms |
| 20 | 3.9 ms | 60.9 ms |

Ferroni allocates nothing per match (the `regex` crate allocates once per
match), so the allocator was not the cause. Every `Captures` cloned the regex's
`Arc` to support `Captures::name`, and every clone and drop wrote the same
shared reference count, which moved one cache line between the cores for each
match. A second shared write sat in the adaptive seek gate of `captures()`,
which updated its rate and its probe counter on every search.

[sebastian-software/ferroni#417](https://github.com/sebastian-software/ferroni/pull/417)
keeps the `Arc` only for patterns with named groups, writes the gate's shared
state only when it changes, and samples the closed gate's probes per thread.
With it, `captures_iter` scales like `find_iter` (7.3 ms at 20 threads instead
of 60.9 ms), and the regex backend's full check drops from 86.7 ms to 29.5 ms.
The released Ferroni 2.2.3 that ferrolink depends on does not contain the fix
yet.

A profile of the single-thread run puts about 82% of the time in the
backtracking matcher itself and about 10% in the capture API around it: the
remaining gap to html5gum is the cost of trying a four-way alternation at every
`<`, not a defect.

## Decision

html5gum is the default and the only backend in the default build: the
fastest on the real workload, a complete WHATWG tokenizer with full character
reference decoding, no `unsafe`, and one small dependency. The other backends
stay behind the `parser-comparison` feature so the comparison can be repeated
on other builds:

- **lol_html** comes close in speed but returns attribute values undecoded and
  brings four MPL-2.0 crates through Servo's selector stack.
- **html5ever** is the reference for correctness but half as fast without a
  tree builder, and slower still with one.
- **Ferroni** is fast as a regex engine and, with the fix above, close to the
  tokenizers on all cores. Like any regex approach it follows a browser only
  on well-formed markup, and it decodes only the character references
  ferrolink's small table knows.
