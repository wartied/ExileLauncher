#!/usr/bin/env bash
# Copies the client source into the frozen benchmark copy, keeping its run
# folder (bench world, options) so results stay comparable between runs.
#
#   tools/freeze.sh
set -euo pipefail
BENCH_DIR=${EXILE_BENCH_DIR:-"$LOCALAPPDATA/exile-dev/bench-client"}
SRC=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$BENCH_DIR"
rm -rf "$BENCH_DIR/src" "$BENCH_DIR/modmenu-shim"
(cd "$SRC" && tar --exclude=./build --exclude=./modmenu-shim/build --exclude=./run --exclude=./.gradle --exclude=./tools -cf - .) | (cd "$BENCH_DIR" && tar -xf -)
# The jar task bundles ../LICENSE.
cp "$SRC/../LICENSE" "$BENCH_DIR/../LICENSE" 2>/dev/null || true
echo "Frozen client copy updated in $BENCH_DIR"
