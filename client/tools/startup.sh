#!/usr/bin/env bash
# Starts the frozen client, waits for the first menu, and prints how long the
# game took from JVM start (-Dexile.startup.probe quits right after).
#
#   tools/startup.sh <label> [JVM args...]
set -euo pipefail
label=$1; shift
BENCH_DIR=${EXILE_BENCH_DIR:-"$LOCALAPPDATA/exile-dev/bench-client"}
export JAVA_HOME=${EXILE_JAVA_HOME:-$(ls -d "$LOCALAPPDATA"/exile-dev/jdk-25* 2>/dev/null | tail -1)}
LOG_DIR="$BENCH_DIR/logs"
mkdir -p "$LOG_DIR"
cd "$BENCH_DIR"
timeout 600 ./gradlew --no-daemon -q :runClient ${EXILE_GRADLE_ARGS:-} \
	-PexileArgs="-Dexile.startup.probe=true $*" > "$LOG_DIR/startup-$label.log" 2>&1 || true
ms=$(grep -o "menu reached [0-9]* ms" "$LOG_DIR/startup-$label.log" | grep -o "[0-9]*" || true)
echo "$label ${ms:-FAILED}"
