import { getEslintConfig } from "eslint-config-setup";

const config = await getEslintConfig({ node: true, oxlint: true });

config.unshift({
  ignores: [
    "**/dist/**",
    "coverage/**",
    "node_modules/**",
    "pnpm-lock.yaml",
    "**/*.json",
    "**/*.md",
    // Generated from the Rust binding by `pnpm build:native`.
    "src/binding.d.ts",
  ],
});

config.push({
  files: ["scripts/**/*.ts", "test/**/*.ts"],
  rules: {
    // The scripts and tests write to paths they compute, by design.
    "security/detect-non-literal-fs-filename": "off",
  },
});

config.push({
  files: ["test/**/*.ts"],
  rules: {
    // node:test's `describe` and `test` return promises the runner awaits.
    "@typescript-eslint/no-floating-promises": [
      "error",
      {
        allowForKnownSafeCalls: [
          { from: "package", name: ["describe", "test"], package: "node:test" },
        ],
      },
    ],
  },
});

export default config;
