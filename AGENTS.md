# AGENTS.md

ferrolink is a Rust CLI that checks links, assets, and anchors in static HTML
builds, first of all Ardo documentation sites. Project language is US English.

## Layout

- `src/check.rs`: the pipeline (walk with ferralk, scan with rayon, check).
- `src/scan/`: one collector decides what is a link or an anchor; the parser
  backends only tokenize. html5gum is the default and the only backend in the
  default build; html5ever, lol_html, and Ferroni sit behind the
  `parser-comparison` feature for `examples/compare_parsers.rs`.
- `src/resolve.rs` and `src/site.rs`: browser URL resolution and static-host
  URL-to-file mapping.
- `src/deploy.rs`: `versions.json`, `_redirects`, and `vercel.json`.
- `tests/fixtures/site`: the end-to-end fixture `tests/cli.rs` checks.

## Checks

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
```

A change to what counts as a link must keep every backend in agreement:
`src/scan/tests.rs` and `tests/cli.rs` compare them. Performance claims need a
measurement on a real build (`examples/compare_parsers.rs`, `--timings`).

Commits follow Conventional Commits without a scope.

<!-- sebastian-software-consumer-agents:start -->

# Standards-managed repo guardrails

- Do not hand-edit managed files or standards-owned marker sections.
- If `standards check` reports drift, run `standards apply` or update standards.
- The repository's own gate may omit `standards check`; CI can still fail on it.

Node repositories:

- Fix or format every file reported by `oxfmt` whenever practical.
- For generated files, prefer formatting in the generator step.
- If formatting is not viable, use repo-local `.prettierignore`.
- Never add repo-specific ignores to managed `.oxfmtrc.json`.

Rust repositories:

- Keep `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` green.
- Lint levels belong in `[workspace.lints]`, never in managed `rustfmt.toml`.
- `rust-version` in `Cargo.toml` is the only MSRV; every other mention is a
  derived copy.
- Record a cargo-deny finding as a narrow, commented exception in `deny.toml` —
  never by widening the org allow-list.

<!-- sebastian-software-consumer-agents:end -->
