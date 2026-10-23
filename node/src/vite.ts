/**
 * A Vite plugin that checks the links, assets, and anchors of the build
 * output once the build is complete.
 *
 * ```ts
 * import ferrolink from "ferrolink/vite"
 *
 * export default defineConfig({
 *   plugins: [ferrolink({ siteUrl: "https://example.org/", level: "warn" })],
 * })
 * ```
 *
 * @module
 */

import type { BuildEnvironment, Plugin, ResolvedConfig } from "vite";

import path from "node:path";

import { check, type CheckOptions, formatIssues, type Report } from "./index.js";

/** Options of the ferrolink Vite plugin: the check options without `root`, and how to react. */
export type FerrolinkPluginOptions = {
  /**
   * The directory to check, relative to Vite's `root`. Defaults to the
   * `build.outDir` of the client environment.
   */
  dir?: string;
  /**
   * `"error"` fails the build when the check finds issues; `"warn"` logs them
   * and lets the build pass. Default `"error"`.
   */
  level?: "error" | "warn";
} & Omit<CheckOptions, "root">;

/**
 * Checks the client build output after the build.
 *
 * In an app build (`vite build` with `builder` config or `--app`), frameworks
 * such as Ardo render their HTML in `buildApp`, after the environments are
 * bundled. The check therefore runs in a `buildApp` hook ordered `post`, after
 * every other hook. Without the app builder it runs when the client bundle
 * closes. `basePath` defaults to Vite's `base` when that is an absolute path
 * or URL.
 */
export default function ferrolink(options: FerrolinkPluginOptions = {}): Plugin {
  // Set when the last `buildApp` hook found nothing built: Vite then builds
  // every environment itself, after the hooks, and the check has to wait for
  // the client bundle to close.
  let checkOnClose = false;
  // Environments that wrote their bundle since they last closed one. A bundle
  // also closes after a failed build, and then the build's error is the news.
  const written = new WeakSet<object>();

  return {
    name: "ferrolink",
    apply: "build",
    // Without this, every environment gets its own instance, and the state
    // `buildApp` leaves would not reach `closeBundle`.
    sharedDuringBuild: true,
    buildApp: {
      order: "post",
      async handler(builder) {
        const client = clientEnvironment(Object.values(builder.environments));
        if (client === undefined) return;
        if (client.isBuilt) {
          await checkBuild(client.config, options);
        } else {
          checkOnClose = true;
        }
      },
    },
    writeBundle() {
      written.add(this.environment);
    },
    closeBundle: {
      order: "post",
      sequential: true,
      async handler() {
        if (!written.delete(this.environment)) return;
        const { config } = this.environment;
        if (config.consumer !== "client") return;
        // In an app build, the `buildApp` hook checks once every hook is done.
        if (config.builder !== undefined && !checkOnClose) return;
        await checkBuild(config, options);
      },
    },
  };
}

async function checkBuild(config: ResolvedConfig, options: FerrolinkPluginOptions): Promise<void> {
  const { dir, level = "error", ...checkOptions } = options;
  const root = path.resolve(config.root, dir ?? config.build.outDir);
  const basePath = checkOptions.basePath ?? servedPath(config.base);
  const report = await check({
    ...checkOptions,
    ...(basePath === undefined ? {} : { basePath }),
    root,
  });
  const message = describe(report, path.relative(config.root, root) || ".");
  if (report.issues.length === 0) {
    config.logger.info(message);
  } else if (level === "error") {
    throw new Error(message);
  } else {
    config.logger.warn(message);
  }
}

function clientEnvironment(environments: BuildEnvironment[]): BuildEnvironment | undefined {
  const clients = environments.filter((environment) => environment.config.consumer === "client");
  return clients.find((environment) => environment.name === "client") ?? clients[0];
}

/** The URL path of Vite's `base`; none for a relative base such as `./`. */
function servedPath(base: string): string | undefined {
  if (base.startsWith("/")) return base;
  return URL.canParse(base) ? new URL(base).pathname : undefined;
}

function describe(report: Report, dir: string): string {
  const { summary } = report;
  const counts = `${plural(summary.pages, "page")}, ${plural(summary.internalLinks, "internal link")} in ${dir}`;
  if (summary.issues === 0) return `[ferrolink] No issues (${counts})`;
  const files = new Set(report.issues.map((issue) => issue.source)).size;
  return `[ferrolink] ${plural(summary.issues, "issue")} in ${plural(files, "file")} (${counts}):\n${formatIssues(report.issues)}`;
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
