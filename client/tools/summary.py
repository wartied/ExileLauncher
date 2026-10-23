"""Summarises Exile Bench results, grouping runs labelled <config>-r<n>.

Usage: python tools/summary.py [label prefix ...]
Reads $EXILE_BENCH_DIR/run/exile/bench/*.json (default %LOCALAPPDATA%/exile-dev/bench-client).
Columns: average FPS, 1% and 0.1% lows (with spread between runs), p99 frame
time, GC count and time, allocation rate (all threads / render thread) and
live heap after a full GC.
"""
import collections
import glob
import json
import os
import statistics
import sys

bench_dir = os.environ.get("EXILE_BENCH_DIR") or os.path.join(os.environ["LOCALAPPDATA"], "exile-dev", "bench-client")
rows = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(bench_dir, "run", "exile", "bench", "*.json"))):
    d = json.load(open(f, encoding="utf-8"))
    rows[d["label"].rsplit("-r", 1)[0]].append(d)

print(f"{'config':22} {'runs':>4} {'avg':>7} {'sd':>4} {'1%low':>7} {'sd':>4} {'0.1%low':>8} "
      f"{'p99ms':>6} {'GCs':>4} {'GCms':>5} {'MB/s':>6} {'rMB/s':>6} {'live':>5}")
for k, ds in rows.items():
    if sys.argv[1:] and not any(k.startswith(a) for a in sys.argv[1:]):
        continue
    m = lambda key: statistics.mean(d.get(key, 0) for d in ds)
    sd = lambda key: statistics.stdev(d[key] for d in ds) if len(ds) > 1 else 0
    print(f"{k:22} {len(ds):4} {m('avg_fps'):7.0f} {sd('avg_fps'):4.0f} {m('low_1pct_fps'):7.0f} "
          f"{sd('low_1pct_fps'):4.0f} {m('low_0_1pct_fps'):8.0f} {m('p99_ms'):6.2f} {m('gc_count'):4.0f} "
          f"{m('gc_ms'):5.0f} {m('alloc_mb_per_s'):6.0f} {m('render_alloc_mb_per_s'):6.0f} {m('live_heap_mb'):5.0f}")
