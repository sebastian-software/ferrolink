#!/usr/bin/env bash
#
# Prepares npm for ferrolink's first release, once, from a maintainer's
# machine. Run it before merging the first release pull request.
#
# The publish workflow authenticates with Trusted Publishing (OIDC), which npm
# configures per package and only for a package that already exists. The real
# packages need the addons that only CI builds, so this script reserves every
# name with a placeholder version 0.0.0 instead, deprecates it, and then trusts
# the publish workflow for each package. The first release then goes out from
# CI, with provenance and without a token.
#
# The packages are `ferrolink` and one `@ferrolink/<platform>` per napi target
# in node/package.json. A package that already exists is not published again,
# and one whose trust is configured is left alone, so the script can be
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

# A version just published can take a moment to show up for other commands.
retry() {
  local attempt
  for attempt in 1 2 3 4 5; do
    if "$@"; then
      return 0
    fi
    echo "  retrying in 5 seconds ($attempt of 5 failed)"
    sleep 5
  done
  return 1
}

# Whether the placeholder exists and is not deprecated yet.
undeprecated_placeholder() {
  [[ "$(npm view "$1@$placeholder" version 2>/dev/null)" == "$placeholder" ]] &&
    [[ -z "$(npm view "$1@$placeholder" deprecated 2>/dev/null)" ]]
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

published=0
trusted=0

# The list comes in on descriptor 3, so npm keeps the terminal as its stdin:
# without one, it cannot wait for the browser login that 2FA asks for and
# fails with EOTP.
while IFS= read -r name <&3; do
  echo
  echo "==> $name"

  fresh=false
  if npm view "$name" name >/dev/null 2>&1; then
    echo "  already on npm"
  else
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
    fresh=true
    published=$((published + 1))
  fi

  if $fresh || undeprecated_placeholder "$name"; then
    if $dry_run; then
      run npm deprecate "$name@$placeholder" "Placeholder; install the latest ferrolink release."
    else
      retry npm deprecate "$name@$placeholder" "Placeholder; install the latest ferrolink release."
    fi
  fi

  # Without `--json`, an existing configuration names its workflow file.
  if ! $dry_run && [[ "$(npm trust list "$name" 2>/dev/null || true)" == *"$workflow"* ]]; then
    echo "  already trusts $workflow"
  else
    run npm trust github "$name" --repository "$repository" --file "$workflow" --allow-publish --yes
    trusted=$((trusted + 1))
    # npm asks for a 2-second pause between trust calls to avoid rate limits.
    $dry_run || sleep 2
  fi
done 3<<<"$packages"

echo
if $dry_run; then
  echo "Dry run: $published package(s) to publish, $trusted trust configuration(s) to create."
else
  echo "Published $published placeholder(s); configured $trusted trusted publisher(s)."
  echo "The next release publishes from $workflow through Trusted Publishing."
fi
