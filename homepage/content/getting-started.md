# Getting started

Ferrolink checks the HTML your static site generator has already written.
Build the site first, then check its output directory. Links, assets and anchors
are checked locally; external URLs are counted without being fetched.

## Start with the command line

Install the Rust CLI, then point it at your finished build:

```bash
cargo install --locked ferrolink
ferrolink check dist
```

Replace `dist` with your generator’s output directory. If your site is served
under a project path, include it in the check:

```bash
ferrolink check dist --site-url https://example.org/docs/
```

The URL supplies the `/docs/` base path and identifies absolute links to your
own host. A link to `/guide/` that forgets `/docs/` is an issue.

An exit code of `0` means the check passed, `1` means it found issues, and `2`
means it could not run. Put the command after the build in CI to stop a
broken artifact before deployment.

[Read the full CLI reference](../../README.md.src#usage), including archived
versions, assumed targets and JSON output.

## Add a Vite build check

The npm package includes prebuilt native binaries, so Node.js users do not
need a Rust toolchain:

```bash
npm install --save-dev ferrolink
```

Add the plugin to your Vite configuration:

```ts
import { defineConfig } from "vite";
import ferrolink from "ferrolink/vite";

export default defineConfig({
  plugins: [ferrolink({ siteUrl: "https://example.org/" })],
});
```

The plugin checks the client output after the build and fails it when issues
are found. Its base path defaults to Vite’s `base`. Frameworks that render
pages in Vite’s `buildApp` phase, including Ardo, are checked after those hooks
have completed.

Node.js 20.19+ or 22.12+ and Vite 7 or 8 are supported. The
[Node.js and Vite reference](../../node/README.md) lists options, native
platforms and the programmatic API.

## Read the result

Every issue identifies its source file, line, reference and resolved target.
A missing file is a `broken_link`; an absent element on an existing page is a
`missing_anchor`; a URL leaving the configured project path is `outside_base`.

The [homepage example](/#example) shows the same small build before and after
fixing all three. Its results are real output of the published npm package,
regenerated and compared in CI.

## Know the boundary

Ferrolink checks the build on disk. It does not test external availability,
run JavaScript or validate the full HTML standard. Its scanner is tuned for
generator output. Known differences on malformed markup and limited named
character-reference decoding are recorded in the
[parser decision](../../docs/parsers.md#decision).

The JavaScript API is the supported integration contract. Before version 1.0,
breaking changes arrive in minor releases; Rust library internals can also
change in any minor release. See the
[stability policy](../../node/README.md#api-stability) before integrating.
