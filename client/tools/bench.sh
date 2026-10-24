#!/usr/bin/env bash
# Runs one unattended Exile Bench and prints its one-line summary.
#
#   tools/bench.sh <label> [JVM args and -D options...]
#   EXILE_GRADLE_ARGS=-PferriteRun tools/bench.sh ferrite-r1 ...   (extra Gradle properties)
#
# Benchmarks run from a frozen copy of the client (see tools/freeze.sh), so
# you can keep editing the source while a series of runs is in progress.
# Results (JSON) land in $EXILE_BENCH_DIR/run/exile/bench; summarise them with
# tools/summary.py. Uses EXILE_JAVA_HOME, or the newest JDK 25 in exile-dev.
set -euo pipefail
label=$1; shift
BENCH_DIR=${EXILE_BENCH_DIR:-"$LOCALAPPDATA/exile-dev/bench-client"}
export JAVA_HOME=${EXILE_JAVA_HOME:-$(ls -d "$LOCALAPPDATA"/exile-dev/jdk-25* 2>/dev/null | tail -1)}
LOG_DIR="$BENCH_DIR/logs"
mkdir -p "$LOG_DIR"
cd "$BENCH_DIR"
timeout 900 ./gradlew --no-daemon -q :runClient ${EXILE_GRADLE_ARGS:-} \
	-PexileArgs="-Dexile.bench=auto -Dexile.bench.label=$label $*" > "$LOG_DIR/bench-$label.log" 2>&1 || true
line=$(grep -h "Exile Bench: [0-9]" "$LOG_DIR/bench-$label.log" | sed "s/^.*Exile Bench: //; s/ -> .*//" || true)
echo "$label: ${line:-FAILED (see $LOG_DIR/bench-$label.log)}"
