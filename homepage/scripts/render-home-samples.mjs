/* eslint-disable security/detect-non-literal-fs-filename -- The script owns its temporary builds and the repository-owned sample destination. */
import { check as runCheck } from "ferrolink";
/**
 * Writes app/data/home-samples.json, everything the homepage shows:
 *
 * - the three check specimens, real output of the published ferrolink npm
 *   package at the exact version pinned in homepage/package.json, run on small
 *   builds this script writes to a temporary directory;
 * - the parser comparison, read from the published tables in docs/parsers.md.
 *
 * `pnpm sample:write` writes the file, `pnpm sample:check` fails when it no
 * longer matches. Nobody edits the file by hand.
 */
import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { format } from "oxfmt";

const checkOnly = process.argv.includes("--check");
const destination = new URL("../app/data/home-samples.json", import.meta.url);
const repository = (path) => new URL(`../../${path}`, import.meta.url);
/** Committed text compared on any checkout, including Windows CRLF ones. */
const normalized = (text) => text.replaceAll("\r\n", "\n");

// The published package that renders the specimens, pinned to one version.
const site = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
const pinned = site.devDependencies.ferrolink;
assert.match(pinned, /^\d+\.\d+\.\d+$/, "Pin ferrolink in homepage/package.json to one version.");
const installed = JSON.parse(
  await readFile(new URL("../node_modules/ferrolink/package.json", import.meta.url), "utf8"),
).version;
assert.equal(installed, pinned, "Run pnpm install: the installed ferrolink is not the pinned one.");
const release = JSON.parse(await readFile(repository(".release-please-manifest.json"), "utf8"))[
  "."
];
if (!checkOnly && pinned !== release) {
  console.warn(
    `Rendering with ferrolink ${pinned}; the current release is ${release}. Update the pin in homepage/package.json once ${release} is on npm.`,
  );
}

/** Writes a small build, checks it with the published package and removes it again. */
async function checkBuild(files, options) {
  const root = await mkdtemp(join(tmpdir(), "ferrolink-homepage-"));
  try {
    for (const [path, text] of Object.entries(files)) {
      await mkdir(join(root, dirname(path)), { recursive: true });
      await writeFile(join(root, path), text);
    }
    const { summary, issues } = await runCheck({ root, threads: 1, ...options });
    return {
      summary,
      issues: issues.map(({ source, line, origin, href, target, kind }) => ({
        source,
        line,
        origin,
        href,
        target,
        kind,
      })),
    };
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}
const kinds = (result) => result.issues.map(({ kind }) => kind);
const page = (title, lines) => `<!doctype html>\n<title>${title}</title>\n${lines.join("\n")}\n`;

// 1. Three small mistakes, before and after fixing them.
const exampleSite = "https://example.org/docs/";
const exampleFiles = {
  "guide/setup.html": page("Setup", ['<h1 id="install">Install</h1>']),
  "assets/logo.svg":
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6"/></svg>\n',
};
const exampleStates = [
  {
    id: "before",
    links: [
      '<a href="/docs/guide/setup#installation">Install</a>',
      '<img src="/docs/assets/preview.svg" alt="Preview">',
      '<a href="/guide/setup">Guide</a>',
    ],
    expected: ["missing_anchor", "broken_link", "outside_base"],
  },
  {
    id: "after",
    links: [
      '<a href="/docs/guide/setup#install">Install</a>',
      '<img src="/docs/assets/logo.svg" alt="Preview">',
      '<a href="/docs/guide/setup">Guide</a>',
    ],
    expected: [],
  },
];
const external = '<a href="https://example.net/">External</a>';
const example = {
  siteUrl: exampleSite,
  files: Object.entries(exampleFiles).map(([path, text]) => ({ path, text })),
  states: [],
};
for (const { id, links, expected } of exampleStates) {
  const index = page("Example", [...links, external]);
  const result = await checkBuild(
    { ...exampleFiles, "index.html": index },
    { siteUrl: exampleSite },
  );
  assert.deepEqual(kinds(result), expected, `The ${id} build reports other issues.`);
  assert.equal(result.summary.externalLinks, 1);
  example.states.push({ id, page: index, ...result });
}

// 2. How URLs map onto the files a static host serves.
const pathLinks = [
  "/docs/guide",
  "/docs/guide.html",
  "/docs/blog/",
  "/docs/blog",
  "https://example.org/docs/blog/",
  "/docs/Guide",
  "/docs/guide/",
  "/guide",
];
const pathPage = page(
  "Paths",
  pathLinks.map((href) => `<a href="${href}">Link</a>`),
);
const pathFiles = {
  "index.html": pathPage,
  "guide.html": page("Guide", ["<h1>Guide</h1>"]),
  "blog/index.html": page("Blog", ["<h1>Blog</h1>"]),
};
const pathResult = await checkBuild(pathFiles, { siteUrl: exampleSite });
const firstLink = 3;
const paths = {
  siteUrl: exampleSite,
  files: Object.keys(pathFiles),
  links: pathLinks.map((href, index) => {
    const issue = pathResult.issues.find(({ line }) => line === firstLink + index);
    if (issue) assert.equal(issue.href, href, "An issue does not belong to its line.");
    return { href, kind: issue?.kind ?? null };
  }),
  summary: pathResult.summary,
};
assert.equal(pathResult.issues.length, paths.links.filter(({ kind }) => kind).length);
assert(
  paths.links.some(({ kind }) => kind === null) && paths.links.some(({ kind }) => kind),
  "The path sample should show links that resolve and links that do not.",
);

// 3. An archived version and the files that tell the host what to serve.
const versionFiles = {
  "v2/index.html": page("Version 2", ['<a href="/v2/guide/">Guide</a>']),
  "v2/guide/index.html": page("Guide", ["<h1>Guide</h1>"]),
  "v1/index.html": page("Version 1", ['<a href="/v1/api/">API</a>']),
  "versions.json": `${JSON.stringify(
    {
      current: "v2",
      versions: [{ path: "/v2/" }, { path: "/v1/" }, { path: "/v0/" }],
    },
    null,
    2,
  )}\n`,
  _redirects:
    "/latest/*  /v2/:splat  301\n/start     /v2/guide/  301\n/api       /v2/api/    301\n",
};
const versionStates = [
  { id: "every", options: {}, expected: ["broken_link", "broken_link", "broken_link"] },
  { id: "exclude", options: { exclude: ["v1/**"] }, expected: ["broken_link", "broken_link"] },
  {
    id: "assume",
    options: { exclude: ["v1/**"], assumePresent: ["v0/**"] },
    expected: ["broken_link"],
  },
];
const versions = {
  files: Object.keys(versionFiles).toSorted(),
  redirects: versionFiles._redirects,
  manifest: versionFiles["versions.json"],
  states: [],
};
for (const { id, options, expected } of versionStates) {
  const result = await checkBuild(versionFiles, options);
  assert.deepEqual(kinds(result), expected, `The ${id} check reports other issues.`);
  versions.states.push({ id, options, ...result });
}
assert.deepEqual(
  versions.states.at(-1)?.issues.map(({ source, href }) => [source, href]),
  [["_redirects", "/v2/api/"]],
  "Only the redirect to a missing page should be left.",
);

// 4. The parser comparison, from the published tables in docs/parsers.md.
const parsers = normalized(await readFile(repository("docs/parsers.md"), "utf8"));
/** A Markdown table under a heading, as rows of plain cells. */
function table(heading) {
  const section = parsers.split(`${heading}\n`)[1];
  assert(section, `docs/parsers.md has no "${heading}" section.`);
  const lines = section.split("\n");
  const start = lines.findIndex((line) => line.startsWith("|"));
  const end = lines.findIndex((line, index) => index > start && !line.startsWith("|"));
  return lines.slice(start + 2, end).map((line) =>
    line
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim()),
  );
}
const milliseconds = (cell) => {
  // "50.6" in a column of milliseconds, or "24.8 ms ± 0.8".
  const [value, unit] = cell.split(" ");
  const number = Number(value);
  assert(Number.isFinite(number) && (unit === undefined || unit === "ms"), `Not a time: ${cell}`);
  return number;
};
const BACKENDS = ["Ferroni", "html5gum", "lol_html", "html5ever"];
function task(id, rows) {
  const time = (name) => {
    const row = rows.find(([label]) => label === name);
    assert(row, `The ${id} table has no ${name} row.`);
    return milliseconds(row[1]);
  };
  const own = time("Ferroni");
  return {
    id,
    ferroni: own,
    rows: BACKENDS.slice(1).map((name) => ({
      id: name,
      value: time(name),
      ratio: Number((time(name) / own).toFixed(2)),
    })),
  };
}
const workload = parsers.split("## Workload\n")[1]?.split("\n## ")[0]?.replaceAll("\n", " ");
assert(workload, "docs/parsers.md has no workload section.");
const build = /artifact of (\d{4}-\d{2}-\d{2}): ([\d,]+) pages, ([\d.]+) MB of HTML/.exec(workload);
const machine = /(Apple M\d \w+)/.exec(workload)?.[1];
const measuredWith = /Ferrolink (\d+\.\d+\.\d+)/.exec(workload)?.[1];
const revision = /commit `([0-9a-f]{7,})`/.exec(workload)?.[1];
const ferroni = /on Ferroni (\d+\.\d+\.\d+)\./.exec(parsers)?.[1];
const threads = /\(hyperfine, \d+\s+runs, (\d+) threads/.exec(parsers)?.[1];
assert(build && machine && measuredWith && revision && ferroni && threads);
const speed = {
  measured: {
    date: build[1],
    pages: Number(build[2].replaceAll(",", "")),
    megabytes: Number(build[3]),
    machine,
    ferrolink: measuredWith,
    ferroni,
    revision,
    threads: Number(threads),
  },
  tasks: [
    task("scan", table("### Scan throughput, one thread")),
    task(
      "check",
      table("### End to end, all cores").filter(([label]) => !label.includes("one thread")),
    ),
  ],
};

const samples = {
  note: "Generated by homepage/scripts/render-home-samples.mjs. Do not edit.",
  renderedWith: { ferrolink: pinned },
  example,
  paths,
  versions,
  speed,
};
const formatted = await format("home-samples.json", JSON.stringify(samples));
assert.equal(formatted.errors.length, 0);
if (checkOnly) {
  assert.equal(
    normalized(await readFile(destination, "utf8")),
    formatted.code,
    "The homepage samples changed. Run pnpm sample:write and commit the result.",
  );
  console.log(`The homepage samples match ferrolink ${pinned} and docs/parsers.md.`);
} else {
  await writeFile(destination, formatted.code);
  console.log(`Wrote app/data/home-samples.json with ferrolink ${pinned}.`);
}
