// Assembles the npm packages of a release in the publish job: one
// `@ferrolink/<platform>` package per napi target in package.json, holding the
// addon CI built for it, and those packages as optional dependencies of
// `ferrolink`.
//
//   node scripts/prepare-publish.ts <artifact directory>
//
// The artifact directory holds `native-<platform>/ferrolink.<platform>.node`
// for every target, as the build jobs upload them. The script prints the
// package directories still to publish, the platform packages first, and
// writes them to the `packages` step output on GitHub Actions. A package whose
// version is already on npm is left out, so a failed publish can be retried.

import { parseTriple } from "@napi-rs/cli";
import {
  appendFileSync,
  copyFileSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

type Manifest = {
  engines: Record<string, string>;
  name: string;
  license: string;
  napi: { binaryName: string; targets: string[] };
  optionalDependencies?: Record<string, string>;
  repository: unknown;
  version: string;
};

const operatingSystems: Record<string, string> = {
  darwin: "macOS",
  linux: "Linux",
  win32: "Windows",
};

const packageDir = path.resolve(import.meta.dirname, "..");
const license = path.resolve(packageDir, "..", "LICENSE");
const [artifactArgument] = process.argv.slice(2);
if (artifactArgument === undefined) {
  throw new Error("usage: node scripts/prepare-publish.ts <artifact directory>");
}
const artifacts = path.resolve(artifactArgument);
const manifestPath = path.join(packageDir, "package.json");
// eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- this package's own manifest
const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as Manifest;
const targets = manifest.napi.targets.map((triple) => parseTriple(triple));

const expected = new Set(targets.map((target) => `native-${target.platformArchABI}`));
const unexpected = readdirSync(artifacts).filter((name) => !expected.has(name));
if (unexpected.length > 0) {
  throw new Error(`Artifacts without a napi target in package.json: ${unexpected.join(", ")}`);
}

const optionalDependencies: Record<string, string> = {};
const packages = new Map<string, string>();
for (const target of targets) {
  const id = target.platformArchABI;
  const name = `@ferrolink/${id}`;
  const binary = `${manifest.napi.binaryName}.${id}.node`;
  const dir = path.join(packageDir, "npm", id);
  mkdirSync(dir, { recursive: true });
  // Fails when the build of a target is missing.
  copyFileSync(path.join(artifacts, `native-${id}`, binary), path.join(dir, binary));
  copyFileSync(license, path.join(dir, "LICENSE"));
  const libc = { gnu: "glibc", musl: "musl" }[target.abi ?? ""];
  const platform = `${operatingSystems[target.platform] ?? target.platform} ${target.arch}${libc === "musl" ? " (musl)" : ""}`;
  writeJson(path.join(dir, "package.json"), {
    name,
    version: manifest.version,
    description: `The ${platform} binary of ferrolink. Install \`ferrolink\`, which depends on it.`,
    license: manifest.license,
    repository: manifest.repository,
    files: [binary, "LICENSE"],
    os: [target.platform],
    cpu: [target.arch],
    ...(libc === undefined ? {} : { libc: [libc] }),
    main: binary,
    engines: manifest.engines,
  });
  optionalDependencies[name] = manifest.version;
  // `./` keeps npm from reading `npm/<platform>` as a GitHub repository.
  packages.set(`./npm/${id}`, name);
}
writeJson(manifestPath, { ...manifest, optionalDependencies });
copyFileSync(license, path.join(packageDir, "LICENSE"));
packages.set(".", manifest.name);

const pending: string[] = [];
for (const [directory, name] of packages) {
  if (await isPublished(name, manifest.version)) {
    console.error(`${name}@${manifest.version} is already on npm; skipping it.`);
  } else {
    pending.push(directory);
  }
}
console.log(pending.join("\n"));
const output = process.env.GITHUB_OUTPUT;
if (output !== undefined && output !== "") {
  appendFileSync(output, `packages<<EOF\n${pending.join("\n")}\nEOF\n`);
}

async function isPublished(name: string, version: string): Promise<boolean> {
  const response = await fetch(
    `https://registry.npmjs.org/${name.replace("/", "%2F")}/${version}`,
    {
      signal: AbortSignal.timeout(10_000),
    },
  );
  if (response.status === 404) return false;
  // Anything else than "found" or "not found" must not be read as either.
  if (!response.ok) throw new Error(`npm answered ${response.status} for ${name}@${version}`);
  return true;
}

function writeJson(file: string, value: unknown): void {
  writeFileSync(file, `${JSON.stringify(value, undefined, 2)}\n`);
}
