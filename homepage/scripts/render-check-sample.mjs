/* eslint-disable security/detect-non-literal-fs-filename -- The script owns a fresh temporary build and the repository-owned sample destination. */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { format } from "oxfmt";

const repository = fileURLToPath(new URL("../../", import.meta.url));
const output = new URL("../app/data/check-sample.json", import.meta.url);
const fixture = await mkdtemp(join(tmpdir(), "ferrolink-site-sample-"));
try {
  await mkdir(join(fixture, "guide"));
  await mkdir(join(fixture, "assets"));
  await writeFile(
    join(fixture, "guide/setup.html"),
    '<!doctype html>\n<title>Setup</title>\n<h1 id="install">Install</h1>\n',
  );
  await writeFile(
    join(fixture, "assets/logo.svg"),
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>',
  );
  const builds = [
    {
      id: "broken",
      label: "Before · three issues",
      source:
        '<a href="/docs/guide/setup#installation">Install</a>\n<img src="/docs/assets/preview.svg" alt="Preview">\n<a href="/guide/setup">Guide</a>',
      kinds: ["missing_anchor", "broken_link", "outside_base"],
      exit: 1,
    },
    {
      id: "fixed",
      label: "After · corrected references",
      source:
        '<a href="/docs/guide/setup#install">Install</a>\n<img src="/docs/assets/logo.svg" alt="Preview">\n<a href="/docs/guide/setup">Guide</a>',
      kinds: [],
      exit: 0,
    },
  ];
  const samples = [];
  for (const { kinds, exit, ...build } of builds) {
    await writeFile(
      join(fixture, "index.html"),
      `<!doctype html>\n<title>Example</title>\n${build.source}\n<a href="https://example.net/">External</a>\n`,
    );
    const result = spawnSync(
      "cargo",
      [
        "run",
        "--quiet",
        "--locked",
        "--manifest-path",
        join(repository, "Cargo.toml"),
        "--",
        "check",
        fixture,
        "--site-url",
        "https://example.org/docs/",
        "--format",
        "json",
      ],
      { cwd: repository, encoding: "utf8" },
    );
    if (result.error) throw result.error;
    assert.equal(result.status, exit, result.stderr);
    const { issues, summary } = JSON.parse(result.stdout);
    assert.deepEqual(
      issues.map(({ kind }) => kind),
      kinds,
    );
    assert.equal(summary.issues, kinds.length);
    assert.equal(summary.pages, 2);
    assert.equal(summary.external_links, 1);
    const lines = issues.map(
      ({ source, line, kind, href }) =>
        `${source}:${line} · ${kind.replaceAll("_", " ")}\n  ${href}`,
    );
    lines.push(
      `${summary.issues === 0 ? "No issues" : `${summary.issues} issues`} · exit ${exit}\n${summary.pages} pages · ${summary.internal_links} internal references\n${summary.external_links} external URL not checked`,
    );
    samples.push({ ...build, exit, issues, summary, output: lines.join("\n\n") });
  }
  const result = await format("check-sample.json", JSON.stringify({ samples }));
  assert.equal(result.errors.length, 0);
  if (process.argv.includes("--check"))
    assert.equal(
      await readFile(output, "utf8"),
      result.code,
      "Refresh the native check sample with pnpm sample:write.",
    );
  else await writeFile(output, result.code);
  console.log("Verified the failing and corrected builds with the actual Ferrolink Rust CLI.");
} finally {
  await rm(fixture, { recursive: true, force: true });
}
