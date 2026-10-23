// Loads the native addon: the `.node` file a local build puts next to this
// module, else the platform package npm installed as an optional dependency.

import { createRequire } from "node:module";

import type * as Generated from "./binding.js";
import type { CheckOptions, Report } from "./index.js";

/** The addon as the public API types it. */
export type Binding = {
  check: (options: CheckOptions) => Promise<Report>;
};

const require = createRequire(import.meta.url);
let binding: Binding | undefined;

export function loadBinding(): Binding {
  binding ??= load();
  return binding;
}

/**
 * The napi platform id, which names both the addon file and the platform
 * package: `darwin-arm64`, `linux-x64-gnu`, `linux-x64-musl`,
 * `win32-x64-msvc`, ….
 */
export function platformId(
  platform: string = process.platform,
  arch: string = process.arch,
  musl: boolean = isMusl(),
): string {
  switch (platform) {
    case "linux": {
      return `linux-${arch}-${musl ? "musl" : "gnu"}`;
    }
    case "win32": {
      return `win32-${arch}-msvc`;
    }
    default: {
      return `${platform}-${arch}`;
    }
  }
}

function isMusl(): boolean {
  if (process.platform !== "linux") return false;
  // glibc reports its version in the process report; musl reports none.
  const report = process.report.getReport() as { header?: { glibcVersionRuntime?: string } };
  return report.header?.glibcVersionRuntime === undefined;
}

function load(): Binding {
  const id = platformId();
  const failures: string[] = [];
  for (const specifier of [`./ferrolink.${id}.node`, `@ferrolink/${id}`]) {
    try {
      // Returning the generated module as `Binding` makes the compiler check
      // that the Rust side still fits the public types.
      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion, security/detect-non-literal-require -- one of the two addon paths above; `require` returns `any`
      return require(specifier) as typeof Generated;
    } catch (error) {
      if (!isNotFound(error, specifier)) failures.push(`${specifier}: ${String(error)}`);
    }
  }
  throw new Error(
    failures.length > 0
      ? `ferrolink cannot load its native binary for ${id}:\n${failures.join("\n")}`
      : `ferrolink has no native binary for ${id}. On a supported platform, reinstall without omitting optional dependencies; ` +
          "the platforms are listed at https://github.com/sebastian-software/ferrolink/tree/main/node#platforms.",
  );
}

function isNotFound(error: unknown, specifier: string): boolean {
  return (
    error instanceof Error &&
    "code" in error &&
    error.code === "MODULE_NOT_FOUND" &&
    error.message.includes(specifier)
  );
}
