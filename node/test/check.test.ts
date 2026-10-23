// The JavaScript API against the built addon, on the same fixture site as
// the CLI tests in tests/cli.rs.

import { check, type Issue } from "ferrolink";
import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, test } from "node:test";
import { fileURLToPath } from "node:url";

import { platformId } from "../dist/native.js";

const site = fileURLToPath(new URL("../../tests/fixtures/site", import.meta.url));

function brief(issues: Issue[]): string[] {
  return issues.map((issue) => `${issue.source}:${issue.line} ${issue.kind} ${issue.target}`);
}

describe("check", () => {
  test("reports every kind of issue with the site URL", async () => {
    const report = await check({ root: site, siteUrl: "https://example.org/docs/" });
    assert.deepEqual(brief(report.issues), [
      "_redirects:2 broken_link /docs/removed/",
      "archive/index.html:3 broken_link /docs/gone",
      "index.html:13 missing_anchor /docs/guide/setup#missing",
      "index.html:14 broken_link /docs/guide/nope",
      "index.html:15 outside_base /guide/setup",
      "index.html:16 broken_link /docs/about",
      "index.html:19 broken_link /docs/assets/logo@2x.svg",
      "versions.json:5 broken_link /docs/v1/",
    ]);
    assert.deepEqual(report.issues[2], {
      source: "index.html",
      line: 13,
      origin: "a[href]",
      href: "guide/setup#missing",
      target: "/docs/guide/setup#missing",
      kind: "missing_anchor",
    });
    const { summary } = report;
    assert.equal(summary.basePath, "/docs/", "the base path comes from the site URL");
    assert.equal(summary.pages, 4);
    assert.equal(summary.checkedPages, 4);
    assert.equal(summary.issues, 8);
    assert.equal(summary.internalLinks, 23);
    assert.equal(summary.externalLinks, 1);
    assert.equal(summary.skippedLinks, 1);
    assert.deepEqual(Object.keys(report.timings), ["walkMs", "scanMs", "checkMs"]);
  });

  test("counts absolute URLs as external without a site URL", async () => {
    const report = await check({ root: site, basePath: "/docs/" });
    assert.equal(report.issues.length, 7);
    // The canonical link and the absolute link to the own site join the external one.
    assert.equal(report.summary.externalLinks, 3);
  });

  test("checks excluded pages' anchors but not their links", async () => {
    const report = await check({ root: site, basePath: "/docs/", exclude: ["archive/**"] });
    assert.ok(!report.issues.some((issue) => issue.source.startsWith("archive/")));
    assert.equal(report.summary.checkedPages, 3);
  });

  test("accepts targets assumed present", async () => {
    const report = await check({ root: site, basePath: "/docs/", assumePresent: ["v1/**"] });
    assert.ok(!report.issues.some((issue) => issue.source === "versions.json"));
    assert.equal(report.issues.length, 6);
  });

  test("skips anchors and deployment files on request", async () => {
    const report = await check({
      root: site,
      basePath: "/docs/",
      anchors: false,
      deployFiles: false,
    });
    assert.deepEqual([...new Set(report.issues.map((issue) => issue.kind))].sort(), [
      "broken_link",
      "outside_base",
    ]);
    assert.ok(
      !report.issues.some((issue) => ["_redirects", "versions.json"].includes(issue.source)),
    );
  });

  test("passes a clean site", async () => {
    const root = await mkdtemp(path.join(tmpdir(), "ferrolink-"));
    await writeFile(path.join(root, "index.html"), '<a href="about#team">About</a>');
    await writeFile(path.join(root, "about.html"), '<h2 id="team">Team</h2><a href="/">Home</a>');
    const report = await check({ root, threads: 1 });
    assert.deepEqual(report.issues, []);
    assert.equal(report.summary.pages, 2);
  });

  test("rejects when the check cannot run", async () => {
    await assert.rejects(check({ root: "/definitely/not/here" }), {
      code: "GenericFailure",
      message: /^cannot check \/definitely\/not\/here/,
    });
    await assert.rejects(check({ root: site, assumePresent: ["v1/["] }), {
      message: /^invalid assume-present pattern/,
    });
    await assert.rejects(check({ root: site, threads: 0 }), { code: "InvalidArg" });
    // @ts-expect-error -- `root` is required
    await assert.rejects(check({}), { code: "InvalidArg" });
  });
});

describe("platformId", () => {
  test("names the addon and the platform package the way napi does", () => {
    assert.equal(platformId("darwin", "arm64", false), "darwin-arm64");
    assert.equal(platformId("linux", "x64", false), "linux-x64-gnu");
    assert.equal(platformId("linux", "arm64", true), "linux-arm64-musl");
    assert.equal(platformId("win32", "x64", false), "win32-x64-msvc");
  });
});
