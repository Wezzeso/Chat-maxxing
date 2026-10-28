#!/usr/bin/env bash
set -euo pipefail
relay_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$relay_root"
: "${GITHUB_REPOSITORY:?Run this script in GitHub Actions with GITHUB_REPOSITORY set}"
: "${GITHUB_SHA:?GITHUB_SHA must identify the release commit}"
: "${GH_TOKEN:?GH_TOKEN is required to publish a release}"
relay_version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
if [[ ! "$relay_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  printf 'Unsupported package version: %s\n' "$relay_version" >&2
  exit 1
fi
relay_tag="v$relay_version"
relay_releases=$(gh api --paginate "repos/$GITHUB_REPOSITORY/releases?per_page=100" --jq '.[].tag_name')
if printf '%s\n' "$relay_releases" | grep -Fxq -- "$relay_tag"; then
  printf 'Release %s already exists; leaving it unchanged.\n' "$relay_tag"
  exit 0
fi
relay_refs=$(gh api "repos/$GITHUB_REPOSITORY/git/matching-refs/tags/$relay_tag" --jq '.[].ref')
if printf '%s\n' "$relay_refs" | grep -Fxq -- "refs/tags/$relay_tag"; then
  printf 'Tag %s already exists without a release; refusing to move or replace it.\n' "$relay_tag" >&2
  exit 1
fi
if [[ "$(git rev-parse HEAD)" != "$GITHUB_SHA" ]]; then
  printf '%s\n' 'Checkout must match GITHUB_SHA.' >&2
  exit 1
fi
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
install -Dm755 target/release/relay bin/relay
bin/relay --doctor
relay_stage=$(mktemp -d "${RUNNER_TEMP:-/tmp}/relay-release.XXXXXX")
trap 'rm -rf "$relay_stage"' EXIT HUP INT TERM
relay_dist="$relay_stage/dist"
mkdir -p "$relay_dist" "$relay_stage/relay"
export SOURCE_DATE_EPOCH
SOURCE_DATE_EPOCH=$(git show -s --format=%ct "$GITHUB_SHA")
bash packaging/build-deb.sh "$relay_dist/relay-native_${relay_version}_amd64.deb"
# git archive includes only committed source, never profiles, credentials, .git or build output.
git archive "$GITHUB_SHA" | tar -x -C "$relay_stage/relay"
install -Dm755 bin/relay "$relay_stage/relay/bin/relay"
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
  -C "$relay_stage" -cf - relay | gzip -n > "$relay_dist/relay-linux-x86_64.tar.gz"
(
  cd "$relay_dist"
  sha256sum relay-native_*.deb relay-linux-x86_64.tar.gz > SHA256SUMS
)
cat > "$relay_stage/notes.md" <<NOTES
Native Linux preview built from commit $GITHUB_SHA.

- Rust application with native GTK controls and embedded WebKitGTK.
- Minimal dark logo sidebar; service pages load on demand.
- Loaded views stay ready for ten minutes after switching away.
- Small windows use a 56 px bottom bar with five equally spaced service icons and 44 px hit areas.
- WhatsApp's mobile layout keeps normal-size text, compact headers, independent scrolling, and the composer at the bottom.
- Back or Escape returns to chats; a menu button opens the retained navigation controls. Wide windows restore the provider layout.
- Right-click the WhatsApp logo to toggle Mobile layout, enabled by default.
- Closing the window exits the app and its engine workers.

Downloads: Debian/Ubuntu installer, portable Linux x86-64 archive, and SHA256 checksums.
Requires GTK 3.24+, WebKitGTK 4.1 (WebKit 2.42+), and glibc 2.34+.

Account sign-in, messaging, calls, and provider authentication flows remain unverified.
Providers may restrict embedded browsers, particularly Google account sign-in.
The WhatsApp layout adapter uses undocumented page structure and falls back to the regular page when its expected hooks are unavailable.
NOTES
gh release create "$relay_tag" --repo "$GITHUB_REPOSITORY" --target "$GITHUB_SHA" \
  --title "$relay_tag — Native Linux preview" --prerelease \
  --notes-file "$relay_stage/notes.md" \
  "$relay_dist/relay-native_${relay_version}_amd64.deb" \
  "$relay_dist/relay-linux-x86_64.tar.gz" "$relay_dist/SHA256SUMS"
