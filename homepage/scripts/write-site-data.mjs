/* eslint-disable security/detect-non-literal-fs-filename -- Source and output paths come from the repository-owned document manifest. */
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { posix } from "node:path";
import { format } from "oxfmt";

import { documents } from "./site-pages.mjs";

const repository = new URL("../../", import.meta.url);
const routes = new URL("../app/routes/", import.meta.url);
const data = new URL("../app/data/", import.meta.url);
const destinations = new Map(documents.map(({ source, path }) => [source, `/${path}/`]));
destinations.set("README.md", "/reference/cli/");
const repoUrl = "https://github.com/sebastian-software/ferrolink";
await mkdir(data, { recursive: true });
for (const section of ["guide", "reference", "evidence"]) {
  await rm(new URL(`${section}/`, routes), { recursive: true, force: true });
}

for (const document of documents) {
  const source = await readFile(new URL(document.source, repository), "utf8");
  const body = source.startsWith("---\n") ? source.slice(source.indexOf("\n---\n") + 5) : source;
  const content = rewriteLinks(body.replace(/^# .+\n/, ""), document.source);
  const destination = new URL(`${document.path}.md`, routes);
  await mkdir(new URL(".", destination), { recursive: true });
  await writeFile(destination, `---\ntitle: ${JSON.stringify(document.title)}\n---\n${content}`);
}

const cargo = await readFile(new URL("Cargo.toml", repository), "utf8");
const version = /^version = "([^"]+)"/m.exec(cargo)?.[1];
if (!version) throw new Error("The package has no release version.");
const rustVersion = /^rust-version = "([^"]+)"$/m.exec(cargo)?.[1];
if (!rustVersion) throw new Error("Cargo.toml must declare the minimum supported Rust version.");
const license = /^license = "([^"]+)"$/m.exec(cargo)?.[1];
if (!license) throw new Error("Cargo.toml must declare the license.");
const npm = JSON.parse(await readFile(new URL("node/package.json", repository), "utf8"));
const node = npm.engines?.node;
if (typeof node !== "string") throw new Error("node/package.json must declare its Node.js range.");
// "^20.19.0 || >=22.12.0" reads as "20.19+ or 22.12+" on the page.
const nodeVersions = node.split("||").map((range) => {
  const [major, minor] = range.replaceAll(/[^\d.]/g, "").split(".");
  if (!major || !minor) throw new Error(`Cannot read the Node.js range ${node}.`);
  return `${major}.${minor}+`;
});
const supportMarks = new Set(["✓", "—"]);
const readme = await readFile(new URL("README.md.src", repository), "utf8");
await writeData("site.json", {
  version,
  rustVersion,
  license,
  nodeVersions,
  auditComparison: readAuditComparison(readme),
});
const labels = {
  guide: "Guide",
  reference: "Reference",
  evidence: "Evidence",
  changelog: "Changelog",
};
await writeData(
  "navigation.json",
  Object.entries(labels).map(([id, label]) => {
    const pages = documents
      .filter((document) => document.section === id)
      .map(({ path, title }) => ({ title, to: `/${path}/` }));
    return { id, label, to: pages[0].to, pages };
  }),
);
async function writeData(name, value) {
  const result = await format(name, JSON.stringify(value));
  if (result.errors.length) throw new Error(`Cannot format ${name}`);
  await writeFile(new URL(name, data), result.code);
}

/** The README supplies the groups and support marks shown on both surfaces. */
function readAuditComparison(markdown) {
  const columns = ["Capability", "Ferrolink", "Search Quality Kit"];
  const lines = markdown.split("\n");
  const heading = lines.findIndex((line) => comparisonCells(line).join("|") === columns.join("|"));
  if (heading === -1) {
    throw new Error("README.md.src is missing the Search Quality Kit comparison.");
  }
  const groups = readComparisonGroups(lines.slice(heading + 2));
  if (groups.length === 0 || groups.some(({ rows }) => rows.length === 0)) {
    throw new Error("The Search Quality Kit comparison needs nonempty groups.");
  }
  return { columns, groups };
}

function readComparisonGroups(lines) {
  const groups = [];
  for (const line of lines) {
    if (!line.startsWith("|")) break;
    const row = readComparisonRow(line);
    if ("rows" in row) {
      groups.push(row);
      continue;
    }
    const group = groups.at(-1);
    if (group === undefined) {
      throw new Error(`Expected a group before ${row.capability}`);
    }
    group.rows.push(row);
  }
  return groups;
}

function readComparisonRow(line) {
  const [capability, ferrolink, kit, extra] = comparisonCells(line);
  if (!capability || ferrolink === undefined || kit === undefined || extra !== undefined) {
    throw new Error(`Expected three cells in comparison row ${line}`);
  }
  if (ferrolink === "" && kit === "") {
    return { name: capability, rows: [] };
  }
  if (!supportMarks.has(ferrolink) || !supportMarks.has(kit)) {
    throw new Error(`Expected support marks for ${capability}`);
  }
  return { capability, ferrolink: ferrolink === "✓", kit: kit === "✓" };
}

function comparisonCells(line) {
  return line
    .split("|")
    .slice(1, -1)
    .map((cell) => cell.trim());
}

function rewriteLinks(markdown, source) {
  let fence;
  return markdown
    .split("\n")
    .map((line) => {
      const delimiter = /^\s*(`{3,}|~{3,})/.exec(line);
      if (delimiter) {
        if (!fence)
          fence = {
            character: delimiter[1][0],
            length: delimiter[1].length,
            rust: line.slice(delimiter[0].length).trim() === "rust",
          };
        else if (delimiter[1][0] === fence.character && delimiter[1].length >= fence.length)
          fence = undefined;
        return line;
      }
      if (fence) return fence.rust && /^#(?: |$)/.test(line) ? null : line;
      return line.replaceAll(/(?<=\]\()[^\s)]+/g, (href) => resolveLink(href, source));
    })
    .filter((line) => line !== null)
    .join("\n");
}

function resolveLink(href, source) {
  if (/^(?:[a-z][a-z0-9+.-]*:|\/|#)/i.test(href)) return href;
  const [file, fragment] = href.split("#");
  const target = posix.normalize(posix.join(posix.dirname(source), file));
  const destination = destinations.get(target);
  if (destination) {
    return `${destination}${fragment ? `#${fragment}` : ""}`;
  }
  const kind = file.endsWith("/") || !posix.extname(file) ? "tree" : "blob";
  return `${repoUrl}/${kind}/main/${target}${fragment ? `#${fragment}` : ""}`;
}
