// Builds the native addon into dist/ and regenerates src/binding.d.ts from
// the Rust binding. Extra arguments go to `napi build`, for example
// `--target aarch64-unknown-linux-musl --cross-compile`.

import { execFileSync } from "node:child_process";

function run(command: string, args: string[]): void {
  // The binaries in node_modules/.bin are .cmd shims on Windows.
  execFileSync(command, args, { stdio: "inherit", shell: process.platform === "win32" });
}

run("napi", [
  "build",
  "--platform",
  "--release",
  "--manifest-path",
  "native/Cargo.toml",
  "--output-dir",
  "dist",
  "--dts",
  "../src/binding.d.ts",
  "--no-js",
  "--no-const-enum",
  ...process.argv.slice(2),
]);
run("oxfmt", ["src/binding.d.ts"]);
