/* eslint-disable security/detect-non-literal-fs-filename -- Every page comes from the repository-owned route manifest. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { expectedPages } from "./site-pages.mjs";

const repository = fileURLToPath(new URL("../../", import.meta.url));
const output = new URL("../build/client/", import.meta.url);
/**
 * Monografie reuses three Ferra identity parts: the mark, the instrument and the
 * workshop logo. Nothing else from the Ferra entry may reach a page.
 */
const ferraIdentity = new Set(["ferra-mark", "ferra-instrument", "ferra-workshop-logo"]);
const { version } = JSON.parse(
  await readFile(new URL("../app/data/site.json", import.meta.url), "utf8"),
);
for (const page of expectedPages) {
  const source = await readFile(new URL(page, output), "utf8");
  const html = source.replaceAll(/<!--[\s\S]*?-->/g, "");
  assert.equal([...html.matchAll(/<h1\b/g)].length, 1, `${page} needs exactly one h1`);
  assert.equal(
    [...html.matchAll(/<main\b|role="main"/g)].length,
    1,
    `${page} needs exactly one main landmark`,
  );
  for (const fragment of [
    'class="mono-header"',
    'class="mono-footer"',
    `<span class="mono-release" title="Current release">v${version}</span>`,
    "https://ferramenta.dev",
  ]) {
    assert(html.includes(fragment), `${page} is missing ${fragment}`);
  }
  assert(!html.includes("site.css"), `${page} uses project-specific CSS`);
  verifyNoFerra(html, page);
  if (page === "index.html") {
    verifyHomepage(html);
    continue;
  }
  assert(
    html.includes('class="mono mono-docs fl-docs"'),
    `${page} should read on the Monografie documentation page.`,
  );
  for (const fragment of ["mono-ardo-shell", "ardo-sidebar", "mono-prose"]) {
    assert(
      html.includes(fragment),
      `${page} is missing the Monografie reading layout (${fragment})`,
    );
  }
  const blocks = [...html.matchAll(/<pre\b[^>]*>/g)];
  assert(
    blocks.every(([tag]) => tag.includes('tabindex="0"')),
    `${page} has a code block without keyboard scrolling`,
  );
  const tables = [...html.matchAll(/<table\b/g)];
  const regions = [
    ...html.matchAll(/<div\s[^>]*role="region"[^>]*tabindex="0"[^>]*>[\s\S]*?<table\b/g),
  ];
  assert.equal(
    regions.length,
    tables.length,
    `${page} has a table without a keyboard-scrollable region`,
  );
}
await verifyScopedCss();
await verifyNoFerraStyles();
// Dogfood the actual checker for pages, fragments, assets and the project base.
execFileSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--locked",
    "--manifest-path",
    fileURLToPath(new URL("../../Cargo.toml", import.meta.url)),
    "--",
    "check",
    fileURLToPath(output),
    "--site-url",
    "https://ferrolink.dev/",
  ],
  { cwd: repository, stdio: "inherit" },
);
console.log(
  `Verified the shared shell and reading semantics on all ${expectedPages.length} pages.`,
);

/** The homepage reads on the Monografie page and keeps its published anchors. */
function verifyHomepage(html) {
  assert(html.includes('class="mono fl-home"'), "The homepage should use the Monografie page.");
  for (const id of ["overview", "example", "paths", "versions", "scanner", "fit", "start"]) {
    assert(html.includes(`id="${id}"`), `Preserve the published homepage anchor #${id}.`);
  }
  assert(
    html.includes('data-product="ferrolink"'),
    "The homepage opening should show Ferrolink's own instrument.",
  );
}

/** Renders no Ferra class beyond the identity parts Monografie reuses. */
function verifyNoFerra(html, page) {
  const classes = [...html.matchAll(/\bclass="([^"]*)"/g)].flatMap(([, value]) =>
    value.split(/\s+/),
  );
  const foreign = [
    ...new Set(classes.filter((name) => name.startsWith("ferra-") && !ferraIdentity.has(name))),
  ];
  assert.deepEqual(foreign, [], `${page} renders Ferra classes`);
}

/** No Ferra stylesheet is imported: by the site's source, or into a built stylesheet. */
async function verifyNoFerraStyles() {
  const appFiles = await readdir(new URL("../app/", import.meta.url), { recursive: true });
  for (const file of appFiles.filter((name) => /\.(?:tsx?|css|mdx?)$/.test(name))) {
    const source = await readFile(new URL(`../app/${file}`, import.meta.url), "utf8");
    assert.doesNotMatch(
      source,
      /ferramenta-family\/(?:ferra(?:-ardo|-tokens|-fonts)?\.css|ferra["'])/,
      `${file} imports the Ferra entry or its stylesheets`,
    );
  }
  const assets = new URL("assets/", output);
  const assetFiles = await readdir(assets);
  for (const file of assetFiles.filter((name) => name.endsWith(".css"))) {
    const css = await readFile(new URL(file, assets), "utf8");
    const foreign = [
      ...new Set(
        [...css.matchAll(/\.(ferra-[\w-]+)/g)]
          .map(([, name]) => name)
          .filter((name) => !ferraIdentity.has(name)),
      ),
    ];
    assert.deepEqual(foreign, [], `assets/${file} carries Ferra styles`);
  }
}

/** A selector list split at its top-level commas, so `:is(a, b)` stays whole. */
function selectorList(prelude) {
  const selectors = [""];
  let depth = 0;
  for (const char of prelude) {
    if (char === "(") depth += 1;
    if (char === ")") depth -= 1;
    if (char === "," && depth === 0) selectors.push("");
    else selectors[selectors.length - 1] += char;
  }
  return selectors;
}

/**
 * Shared presentation lives in ferramenta-family. The site styles only its own
 * homepage specimens and shell seams, next to their components, under fl-.
 */
async function verifyScopedCss() {
  const appFiles = await readdir(new URL("../app/", import.meta.url), { recursive: true });
  for (const file of appFiles.filter((name) => name.endsWith(".css"))) {
    const path = file.replaceAll("\\", "/");
    assert(
      path.startsWith("components/"),
      `Keep ${path} beside its specimen in app/components/, or in ferramenta-family.`,
    );
    const css = await readFile(new URL(`../app/${path}`, import.meta.url), "utf8");
    const preludes = css
      .replaceAll(/\/\*[\s\S]*?\*\//g, "")
      .split("{")
      .slice(0, -1)
      .map((chunk) =>
        chunk.slice(Math.max(chunk.lastIndexOf("}"), chunk.lastIndexOf(";")) + 1).trim(),
      )
      .filter((prelude) => !prelude.startsWith("@"));
    for (const selector of preludes.flatMap((prelude) => selectorList(prelude))) {
      assert(selector.includes(".fl-"), `${path}: scope "${selector.trim()}" to an fl- class.`);
    }
  }
}
