#!/bin/sh

set -eu

REPO_ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${RELORA_TARGET_DIR:-$REPO_ROOT/target/release}"

cd "$REPO_ROOT"

echo "Building Relora release binaries from source..."
cargo build --release -p relora -p relora-driver-postgres -p relora-driver-mysql -p relora-driver-sqlite

for binary in \
  "$TARGET_DIR/relora" \
  "$TARGET_DIR/relora-driver-postgres" \
  "$TARGET_DIR/relora-driver-mysql" \
  "$TARGET_DIR/relora-driver-sqlite"
do
  if [ ! -x "$binary" ]; then
    echo "Missing expected executable: $binary" >&2
    exit 1
  fi
done

echo "Running non-interactive Relora smoke test (relora paths --json)..."
"$TARGET_DIR/relora" paths --json >/dev/null

echo "Checking sidecar CLI entrypoints..."
"$TARGET_DIR/relora-driver-postgres" --help >/dev/null
"$TARGET_DIR/relora-driver-mysql" --help >/dev/null
"$TARGET_DIR/relora-driver-sqlite" --help >/dev/null

echo "Relora source build smoke test passed."
