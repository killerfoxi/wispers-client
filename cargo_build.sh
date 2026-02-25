#!/bin/bash
set -euo pipefail
OUT="$PWD/$1"
# Resolve symlinks so this works both when invoked directly and via Bazel sh_binary.
cd "$(dirname "$(readlink -f "$0")")"
cargo build
case "$(uname -s)" in
    Darwin) cp target/debug/libwispers_connect.dylib "$OUT" ;;
    *)      cp target/debug/libwispers_connect.so "$OUT" ;;
esac
