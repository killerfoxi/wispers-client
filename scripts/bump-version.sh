#!/bin/bash
set -euo pipefail

# Updates version strings across all package manifests.
#
# Usage: ./scripts/bump-version.sh 0.8.0-rc4
#
# Updates:
#   - wispers-connect/Cargo.toml (package version)
#   - wconnect/Cargo.toml (package version + wispers-connect dep)
#   - wcadm/Cargo.toml (package version)
#   - wrappers/python/pyproject.toml (PEP 440 format)
#   - wrappers/kotlin/build.gradle.kts (default VERSION_NAME)

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.8.0-rc4"
    exit 1
fi

VERSION="$1"
CLIENT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# PEP 440: remove dash before rc/alpha/beta (0.8.0-rc1 -> 0.8.0rc1)
PY_VERSION=$(echo "$VERSION" | sed 's/-rc/rc/; s/-a/a/; s/-b/b/')

echo "==> Bumping versions to $VERSION (PyPI: $PY_VERSION)"

# Cargo: wispers-connect
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" \
    "$CLIENT_DIR/wispers-connect/Cargo.toml"
echo "    wispers-connect/Cargo.toml"

# Cargo: wconnect (package version)
sed -i '' "/^\[package\]/,/^\[/{s/^version = \".*\"/version = \"$VERSION\"/;}" \
    "$CLIENT_DIR/wconnect/Cargo.toml"
# Cargo: wconnect (wispers-connect dependency version)
sed -i '' "s/wispers-connect = { path = \"..\/wispers-connect\", version = \".*\" }/wispers-connect = { path = \"..\/wispers-connect\", version = \"$VERSION\" }/" \
    "$CLIENT_DIR/wconnect/Cargo.toml"
echo "    wconnect/Cargo.toml"

# Cargo: wcadm
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" \
    "$CLIENT_DIR/wcadm/Cargo.toml"
echo "    wcadm/Cargo.toml"

# Python: pyproject.toml
sed -i '' "s/^version = \".*\"/version = \"$PY_VERSION\"/" \
    "$CLIENT_DIR/wrappers/python/pyproject.toml"
echo "    wrappers/python/pyproject.toml ($PY_VERSION)"

# Kotlin: build.gradle.kts (default version in coordinates(...) call)
sed -i '' "/coordinates(/s/?: \"[^\"]*\"/?: \"$VERSION\"/" \
    "$CLIENT_DIR/wrappers/kotlin/build.gradle.kts"
echo "    wrappers/kotlin/build.gradle.kts"

echo ""
echo "==> Done. Verify with: git diff"
