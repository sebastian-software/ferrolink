#!/usr/bin/env bash
#
# Prepares npm for ferrolink's first release, once, from a maintainer's
# machine. Run it before merging the first release pull request.
#
# The publish workflow authenticates with Trusted Publishing (OIDC), which npm
# configures per package and only for a package that already exists. The real
# packages need the addons that only CI builds, so this script reserves every
# name with a placeholder version 0.0.0 instead and then trusts the publish
# workflow for each package. The first release then goes out from CI, with
# provenance and without a token, and replaces the placeholder as `latest`.
#
# It works in two passes. The first publishes every missing placeholder. npm
# processes a new package for a few minutes before anything else can see it,
# so the second pass waits for each new package before it configures the
# trust.
#
# The packages are `ferrolink` and one `@ferrolink/<platform>` per napi target
# in node/package.json. A package that already exists is not published again,
# and one that already trusts the workflow is left alone, so the script can be
# re-run after a failure.
#
#   pnpm install            # in node/, for @napi-rs/cli
#   npm login               # an account with 2FA that may publish to @ferrolink
#   scripts/initial-publish.sh [--dry-run]
#
# --dry-run prints what would be published or configured and changes nothing.

set -euo pipefail

repository="sebastian-software/ferrolink"
workflow="publish.yml"
placeholder="0.0.0"

dry_run=false
case "${1:-}" in
  "") ;;
  --dry-run) dry_run=true ;;
  *)
    echo "usage: $0 [--dry-run]" >&2
    exit 2
    ;;
esac

node_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$node_dir"

run() {
  if $dry_run; then
    echo "  would run: $*"
  else
    "$@"
  fi
}

# Waits up to ten minutes until npm serves the placeholder of a new package.
wait_until_processed() {
  local attempt
  for attempt in $(seq 1 40); do
    if [[ "$(npm view "$1@$placeholder" version 2>/dev/null)" == "$placeholder" ]]; then
      return 0
    fi
    echo "  npm is still processing $1; checking again in 15 seconds"
    sleep 15
  done
  echo "  npm has not made $1 available after ten minutes; run the script again later." >&2
  return 1
}

# Whether the package already trusts the publish workflow of this repository.
# npm prints nothing when the package has no trust configuration, and also
# when listing needs a 2FA login of its own, which it cannot ask for here.
trust_configured() {
  npm trust list "$1" --json 2>/dev/null | node -e '
    let input = ""
    process.stdin.on("data", (chunk) => (input += chunk))
    process.stdin.on("end", () => {
      const [workflow, repository] = process.argv.slice(1)
      let configs = []
      try {
        configs = [].concat(JSON.parse(input))
      } catch {}
      const found = configs.some((c) => c.file === workflow && c.repository === repository)
      process.exit(found ? 0 : 1)
    })
  ' "$workflow" "$repository"
}

# Trusts the publish workflow, retrying for up to five minutes while npm still
# processes a new package. A failed attempt checks once more before it waits:
# npm refuses a second configuration with a conflict, and when the check
# before missed an existing one, the 2FA login of the failed attempt now lets
# the check see it.
trust_workflow() {
  local attempt
  for attempt in $(seq 1 20); do
    if trust_configured "$1"; then
      echo "  already trusts $workflow"
      return 0
    fi
    if npm trust github "$1" --repository "$repository" --file "$workflow" --allow-publish --yes; then
      return 0
    fi
    if trust_configured "$1"; then
      echo "  already trusts $workflow"
      return 0
    fi
    echo "  retrying in 15 seconds ($attempt of 20 failed)"
    sleep 15
  done
  return 1
}

# `npm trust` needs npm 11.15.0 or newer.
if ! node -e '
  const [major, minor] = process.argv[1].split(".").map(Number)
  process.exit(major > 11 || (major === 11 && minor >= 15) ? 0 : 1)
' "$(npm --version)"; then
  echo "npm $(npm --version) is too old for \`npm trust\`; install npm 11.15.0 or newer." >&2
  exit 1
fi

if ! user="$(npm whoami 2>/dev/null)"; then
  if $dry_run; then
    echo "Not logged in to npm; a real run needs \`npm login\` first."
  else
    echo "Log in with \`npm login\` first." >&2
    exit 1
  fi
else
  echo "Publishing as $user."
fi

if [[ ! -d node_modules/@napi-rs/cli ]]; then
  echo "Run \`pnpm install\` in $node_dir first." >&2
  exit 1
fi

# The same names scripts/prepare-publish.ts gives the packages.
packages="$(node --input-type=module -e '
  import { readFileSync } from "node:fs"
  import { parseTriple } from "@napi-rs/cli"
  const manifest = JSON.parse(readFileSync("package.json", "utf8"))
  console.log(manifest.name)
  for (const triple of manifest.napi.targets) {
    console.log(`@ferrolink/${parseTriple(triple).platformArchABI}`)
  }
')"

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
touch "$staging/fresh"

published=0
trusted=0

# Both passes read the list from descriptor 3, so npm keeps the terminal as
# its stdin: without one, it cannot wait for the browser login that 2FA asks
# for and fails with EOTP.

echo
echo "Publishing placeholders"
while IFS= read -r name <&3; do
  if npm view "$name" name >/dev/null 2>&1; then
    echo "  $name: already on npm"
    continue
  fi
  echo "  $name"
  dir="$staging/${name//\//-}"
  mkdir -p "$dir"
  cat >"$dir/package.json" <<EOF
{
  "name": "$name",
  "version": "$placeholder",
  "description": "Reserves the name for ferrolink's first release, which CI publishes. Install the latest ferrolink release instead.",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/$repository.git",
    "directory": "node"
  }
}
EOF
  cat >"$dir/README.md" <<EOF
# $name

A placeholder that reserves this name until ferrolink's first release.
See https://github.com/$repository.
EOF
  (cd "$dir" && run npm publish --access public)
  echo "$name" >>"$staging/fresh"
  published=$((published + 1))
done 3<<<"$packages"

echo
echo "Trusting $workflow"
while IFS= read -r name <&3; do
  echo
  echo "==> $name"
  if $dry_run; then
    if trust_configured "$name"; then
      echo "  already trusts $workflow"
    else
      run npm trust github "$name" --repository "$repository" --file "$workflow" --allow-publish --yes
      trusted=$((trusted + 1))
    fi
    continue
  fi
  if grep -qxF "$name" "$staging/fresh"; then
    wait_until_processed "$name"
  fi
  trust_workflow "$name"
  trusted=$((trusted + 1))
  # npm asks for a 2-second pause between trust calls to avoid rate limits.
  sleep 2
done 3<<<"$packages"

echo
if $dry_run; then
  echo "Dry run: $published package(s) to publish, $trusted trust configuration(s) to create."
else
  echo "Published $published placeholder(s) and checked the trust of $trusted package(s)."
  echo "The next release publishes from $workflow through Trusted Publishing."
fi
