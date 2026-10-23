/**
 * Checks the links, assets, and anchors of a static HTML build with
 * ferrolink's native checker.
 *
 * ```ts
 * import { check } from "ferrolink"
 *
 * const report = await check({ root: "dist", siteUrl: "https://example.org/docs/" })
 * for (const issue of report.issues) {
 *   console.log(`${issue.source}:${issue.line}: ${issue.kind} ${issue.target}`)
 * }
 * ```
 *
 * @module
 */

import { loadBinding } from "./native.js";

/** What to check and how. Only `root` is required. */
export type CheckOptions = {
  /** The build output directory, such as `dist` or `build/client`. Relative paths resolve against the working directory. */
  root: string;
  /**
   * The URL path the directory is served under, such as `/` or `/ardo/` on a
   * GitHub Pages project site. Defaults to the path of `siteUrl`, else `/`.
   */
  basePath?: string;
  /**
   * The public URL of the site. Absolute links to its host (canonical,
   * `og:url`, …) are then checked like paths instead of counted as external.
   */
  siteUrl?: string;
  /**
   * Globs, relative to `root`, of pages whose own links are not checked, such
   * as `v3/**` for an archived version. Links into those pages keep their
   * anchor checks.
   */
  exclude?: string[];
  /**
   * Globs, relative to `root`, of link targets that count as present although
   * the build does not contain them, such as `v5/**` for an archived version
   * that is deployed separately. A glob matches the target path or the
   * `index.html` a host serves for it. Fragments into these targets are not
   * checked.
   */
  assumePresent?: string[];
  /** Whether a `#fragment` must name an element on the target page. Default `true`. */
  anchors?: boolean;
  /** Whether to check `versions.json`, `_redirects`, and `vercel.json` at the root. Default `true`. */
  deployFiles?: boolean;
  /** Worker threads, a positive integer. Defaults to the number of CPUs. */
  threads?: number;
};

/**
 * What is wrong with a reference. New kinds can arrive in a minor release,
 * so treat a kind you do not know as an issue all the same.
 */
export type IssueKind =
  /** No file is served at the target path. */
  | "broken_link"
  /** The target page exists but has no element with the fragment as its `id` (or as the `name` of an `<a>`). */
  | "missing_anchor"
  /** An absolute path that leaves the base path. */
  | "outside_base";

/** One broken reference. */
export type Issue = {
  /** The page or deployment file containing the reference, relative to `root`, with `/` separators. */
  source: string;
  /** One-based line of the element or entry; `0` when unknown. */
  line: number;
  /** Where the reference came from, such as `a[href]` or `_redirects`. Descriptive, not a fixed set. */
  origin: string;
  /** The reference as written, with character references decoded. */
  href: string;
  /** The resolved absolute URL path, including the fragment for anchors. */
  target: string;
  /** What is wrong. */
  kind: IssueKind;
};

/** Counts for the whole run. */
export type Summary = {
  /** HTML pages scanned. */
  pages: number;
  /** Pages whose references were checked (pages not excluded). */
  checkedPages: number;
  /** Files in the build, HTML or not. */
  files: number;
  /** References resolved to a path on the site and checked. */
  internalLinks: number;
  /** References to other hosts, counted but not fetched. */
  externalLinks: number;
  /** References without a document behind them (`mailto:`, `data:`, …). */
  skippedLinks: number;
  /** Number of issues found. */
  issues: number;
  /** The base path the site was checked under, such as `/ardo/`. */
  basePath: string;
};

/** Wall-clock time per phase, in milliseconds. For diagnostics only. */
export type Timings = {
  /** Walking the build directory. */
  walkMs: number;
  /** Reading and parsing every page. */
  scanMs: number;
  /** Resolving and checking every reference. */
  checkMs: number;
};

/** Everything a check found. Broken links are issues here, not errors. */
export type Report = {
  /** Counts for the run. */
  summary: Summary;
  /** Issues ordered by source, line, and reference. */
  issues: Issue[];
  /** Phase durations. */
  timings: Timings;
};

/**
 * Checks every page of the build at `options.root`.
 *
 * The check runs on a worker thread and spreads over every CPU, so the event
 * loop stays free. The promise rejects when the check cannot run: the root is
 * missing, a page cannot be read, a glob is invalid, or this platform has no
 * native binary.
 */
export async function check(options: CheckOptions): Promise<Report> {
  return loadBinding().check(options);
}
