// Real Vite builds with the plugin. Every project gets its own config file,
// as in an application: Vite then loads the config again for each
// environment of an app build, which is what makes the hook timing matter.

import assert from "node:assert/strict";
import { mkdir, mkdtemp, realpath, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, test } from "node:test";
import { build, createBuilder, createLogger, type InlineConfig, type Logger } from "vite";

const plugin = import.meta.resolve("ferrolink/vite");

// A framework that renders pages in `buildApp`, after bundling the client,
// the way Ardo prerenders its routes.
const prerender = `
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"

const prerender = {
  name: "prerender",
  async buildApp(builder) {
    const client = builder.environments.client
    await builder.build(client)
    const outDir = path.resolve(builder.config.root, client.config.build.outDir)
    mkdirSync(path.join(outDir, "about"), { recursive: true })
    writeFileSync(path.join(outDir, "about/index.html"), '<a href="../">Home</a> <a href="/gone/">Gone</a>')
  },
}
`;

type Project = {
  /** Extra statements at the top of the config module. */
  preamble?: string;
  /** The config object, as source. */
  config: string;
};

async function project({ preamble = "", config }: Project): Promise<string> {
  // Vite 7 cannot emit `index.html` from a root behind a symlink, which the
  // temporary directory is on macOS.
  const root = await realpath(await mkdtemp(path.join(tmpdir(), "ferrolink-vite-")));
  await mkdir(path.join(root, "src"));
  await writeFile(
    path.join(root, "index.html"),
    '<!doctype html>\n<a href="/about/">About</a>\n<a href="#top">Top</a>\n<script type="module" src="/src/main.js"></script>\n',
  );
  await writeFile(path.join(root, "src/main.js"), 'document.title = "fixture"\n');
  await writeFile(
    path.join(root, "vite.config.mjs"),
    `import ferrolink from ${JSON.stringify(plugin)}\n${preamble}\nexport default ${config}\n`,
  );
  return root;
}

type Logs = {
  logger: Logger;
  info: string[];
  warn: string[];
};

/** A Vite logger that keeps ferrolink's lines. */
function logs(): Logs {
  const logger = createLogger("info");
  const result: Logs = { logger, info: [], warn: [] };
  logger.info = (message) => {
    if (message.startsWith("[ferrolink]")) result.info.push(message);
  };
  logger.warn = (message) => {
    if (message.startsWith("[ferrolink]")) result.warn.push(message);
  };
  return result;
}

function inline(root: string, { logger }: Logs): InlineConfig {
  return {
    root,
    configFile: path.join(root, "vite.config.mjs"),
    customLogger: logger,
    logLevel: "info",
  };
}

/** What `vite build` does: an app build, with or without the `builder` config. */
async function viteBuild(config: InlineConfig): Promise<void> {
  const builder = await createBuilder(config, null);
  await builder.buildApp();
}

describe("ferrolink/vite", () => {
  // ferrolink comes first in these configs, so only the hook order can make
  // it run after the framework.
  test("checks pages a framework renders in buildApp", async () => {
    const root = await project({
      preamble: prerender,
      config: `{ builder: {}, plugins: [ferrolink({ level: "warn" }), prerender] }`,
    });
    const output = logs();
    await viteBuild(inline(root, output));
    // `/about/` exists only after prerendering; its own broken link is found.
    assert.equal(output.warn.length, 1);
    assert.match(
      output.warn.join("\n"),
      /^\[ferrolink\] 1 issue in 1 file \(2 pages, \d+ internal links in dist\):/,
    );
    assert.match(
      output.warn.join("\n"),
      /\nabout\/index\.html\n {2}line 1 {5}broken link {5}\/gone\/$/,
    );
  });

  test("fails an app build on issues by default", async () => {
    const root = await project({
      preamble: prerender,
      config: `{ builder: {}, plugins: [ferrolink(), prerender] }`,
    });
    await assert.rejects(
      viteBuild(inline(root, logs())),
      /^Error: \[ferrolink\] 1 issue in 1 file/,
    );
  });

  test("checks after Vite builds the environments of an app build itself", async () => {
    const root = await project({
      config: `{ builder: {}, environments: { ssr: { build: { ssr: "src/main.js", outDir: "dist-ssr" } } }, plugins: [ferrolink({ level: "warn" })] }`,
    });
    const output = logs();
    await viteBuild(inline(root, output));
    assert.equal(output.warn.length, 1);
    assert.match(output.warn.join("\n"), /\nindex\.html\n {2}line \d+ +broken link {5}\/about\/$/);
  });

  test("checks a plain vite build when the client bundle closes", async () => {
    const root = await project({ config: `{ plugins: [ferrolink({ level: "warn" })] }` });
    const output = logs();
    await viteBuild(inline(root, output));
    assert.equal(output.warn.length, 1);
    assert.match(output.warn.join("\n"), /broken link {5}\/about\//);
  });

  test("checks a build() call, which runs no buildApp hooks", async () => {
    const root = await project({ config: `{ plugins: [ferrolink()] }` });
    await assert.rejects(build(inline(root, logs())), /\[ferrolink\] 1 issue in 1 file/);
  });

  test("leaves the error of a failed build alone", async () => {
    const root = await project({ config: `{ plugins: [ferrolink()] }` });
    await writeFile(path.join(root, "src/main.js"), "export const = 1\n");
    await assert.rejects(build(inline(root, logs())), (error: Error) => {
      assert.doesNotMatch(error.message, /\[ferrolink\]|cannot check/);
      return true;
    });
  });

  test("takes the base path from Vite's base", async () => {
    const root = await project({
      preamble: prerender,
      config: `{ base: "/docs/", builder: {}, plugins: [ferrolink({ assumePresent: ["gone/**"] }), prerender] }`,
    });
    const output = logs();
    // Below `/docs/`, `/about/` and `/gone/` leave the base path.
    await assert.rejects(viteBuild(inline(root, output)), (error: Error) => {
      assert.match(error.message, /outside base {4}\/about\//);
      assert.match(error.message, /outside base {4}\/gone\//);
      return true;
    });
  });

  test("logs a clean build", async () => {
    const root = await project({
      preamble: prerender,
      config: `{ builder: {}, plugins: [ferrolink({ assumePresent: ["gone/**"] }), prerender] }`,
    });
    const output = logs();
    await viteBuild(inline(root, output));
    assert.deepEqual(output.warn, []);
    assert.match(
      output.info.join("\n"),
      /^\[ferrolink\] No issues \(2 pages, \d+ internal links in dist\)$/,
    );
  });
});
