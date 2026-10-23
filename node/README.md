# ferrolink

**Fast, parallel link checker for static HTML builds, for Node.js and Vite.**

ferrolink reads the output directory of a static site build and checks that
every link, asset reference, and `#fragment` points at something the build
contains. It resolves references the way a browser does and maps URLs onto
files the way static hosts do. It needs no server and no network. This package
runs ferrolink's native checker from a prebuilt binary; no Rust toolchain is
involved.

```bash
npm install --save-dev ferrolink
```

## Vite

```ts
// vite.config.ts
import { defineConfig } from "vite";
import ferrolink from "ferrolink/vite";

export default defineConfig({
  plugins: [ferrolink({ siteUrl: "https://example.org/" })],
});
```

After the build, the plugin checks the client build output and fails the
build when it finds issues:

```text
[ferrolink] 1 issue in 1 file (318 pages, 10845 internal links in dist):
guide/index.html
  line 17    broken link     /storybook
```

| Option     | Default                   | Meaning                                          |
| ---------- | ------------------------- | ------------------------------------------------ |
| `level`    | `"error"`                 | `"warn"` logs the issues and lets the build pass |
| `dir`      | the client `build.outDir` | The directory to check, relative to Vite's root  |
| `basePath` | Vite's `base`             | The URL path the site is served under            |

It also takes `siteUrl`, `exclude`, `assumePresent`, `anchors`,
`deployFiles`, and `threads`, as `check()` does below.

In an app build (`builder` config or `vite build --app`), frameworks such as
Ardo render their HTML in `buildApp`, after the environments are bundled. The
plugin then checks in a `buildApp` hook ordered `post`, after every other
hook, so it sees the rendered pages. Otherwise it checks when the client
bundle closes. Vite 7 and 8 are supported.

## JavaScript API

```ts
import { check } from "ferrolink";

const report = await check({ root: "dist", siteUrl: "https://example.org/docs/" });
for (const issue of report.issues) {
  console.log(`${issue.source}:${issue.line} ${issue.kind} ${issue.href} → ${issue.target}`);
}
```

`check(options)` returns a promise of the report. The check runs on a worker
thread and spreads over every CPU, so the event loop stays free. The promise
rejects only when the check cannot run (missing directory, unreadable page,
invalid glob); broken links are issues in the report.

| Option          | Default                       | Meaning                                                                                                   |
| --------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------- |
| `root`          | required                      | The build output directory                                                                                |
| `basePath`      | the path of `siteUrl`, or `/` | The URL path the directory is served under, such as `/ardo/`                                              |
| `siteUrl`       | none                          | The public URL; absolute links to its host are checked like paths                                         |
| `exclude`       | `[]`                          | Globs, relative to `root`, of pages whose own links are not checked                                       |
| `assumePresent` | `[]`                          | Globs of link targets that count as present, such as `v5/**`; a leading `/` matches the absolute URL path |
| `anchors`       | `true`                        | Whether a `#fragment` must name an element on the target page                                             |
| `deployFiles`   | `true`                        | Whether to check `versions.json`, `_redirects`, and `vercel.json`                                         |
| `threads`       | the number of CPUs            | Worker threads                                                                                            |

The report has three parts:

- `issues`: `{ source, line, origin, href, target, kind }` per broken
  reference, ordered by source and line. `kind` is `"broken_link"`,
  `"missing_anchor"`, or `"outside_base"`.
- `summary`: `pages`, `checkedPages`, `files`, `internalLinks`,
  `externalLinks`, `skippedLinks`, `issues`, and the effective `basePath`.
- `timings`: `walkMs`, `scanMs`, and `checkMs`.

`assumePresent` suits archived versions that are deployed separately, or
generated pages another tool adds later. A glob relative to `root` matches the
target path or the `index.html` a host serves for it. A glob with a leading
`/` matches the absolute URL path, base path included, and also covers targets
outside the base path, such as `/blog/**` for another app on the same host. A
matching target is reported neither as missing nor for a missing fragment.

`formatIssues(issues)` renders issues the way the command line prints them:

```ts
import { check, formatIssues } from "ferrolink";

const report = await check({ root: "dist" });
if (report.issues.length > 0) console.error(formatIssues(report.issues));
```

```text
guide/index.html
  line 17    broken link     storybook → /storybook
```

The types ship with the package.

## Platforms

`ferrolink` installs one of these packages as an optional dependency, the one
that matches the machine:

| Platform           | Package                       |
| ------------------ | ----------------------------- |
| macOS arm64        | `@ferrolink/darwin-arm64`     |
| macOS x64          | `@ferrolink/darwin-x64`       |
| Linux arm64, glibc | `@ferrolink/linux-arm64-gnu`  |
| Linux x64, glibc   | `@ferrolink/linux-x64-gnu`    |
| Linux arm64, musl  | `@ferrolink/linux-arm64-musl` |
| Linux x64, musl    | `@ferrolink/linux-x64-musl`   |
| Windows arm64      | `@ferrolink/win32-arm64-msvc` |
| Windows x64        | `@ferrolink/win32-x64-msvc`   |

An install that omits optional dependencies (`--omit=optional`,
`--no-optional`) leaves the binary out, and `check()` then rejects with a
message naming the package it needs. Node.js 20.19+ or 22.12+ is required.

## API stability

`check()`, `formatIssues()`, the fields of the options and of the report, the
issue kinds, and the `ferrolink/vite` plugin with its options follow semantic
versioning:
before 1.0, a breaking change only comes with a new minor version (0.x.0) and
is called out in the changelog. New options, report fields, and issue kinds
can arrive in any minor version; treat an issue kind you do not know as an
issue. The values of `timings` and `origin`, error messages, and the text of
log output and of `formatIssues()` are not part of the contract.

## More

The [repository](https://github.com/sebastian-software/ferrolink) has the
command-line tool, how URLs map to files, and what exactly is checked.

## License

[Apache-2.0](https://github.com/sebastian-software/ferrolink/blob/main/LICENSE),
© Sebastian Software GmbH.
