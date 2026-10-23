package dev.exile.client.bench;

import com.google.gson.JsonObject;
import com.mojang.blaze3d.systems.RenderSystem;
import dev.exile.client.perf.FrameStats;
import java.util.Arrays;
import net.minecraft.SharedConstants;
import net.minecraft.client.Minecraft;

/** Statistics for one measured bench lap. */
public record BenchResult(
	int frames,
	double seconds,
	double avgFps,
	double onePercentLowFps,
	double pointOnePercentLowFps,
	double p50Ms,
	double p95Ms,
	double p99Ms,
	double maxMs,
	/** Mean absolute change between consecutive frame times: lower = smoother pacing. */
	double jitterMs,
	double[] phaseAvgMs,
	long gcCount,
	long gcMs,
	/** Allocation rate across all threads, MB/s. */
	double allocMbPerSec,
	/** Allocation rate of the render thread alone, MB/s. */
	double renderAllocMbPerSec,
	/** Heap in use right after a full GC before measuring: what the game really holds, MB. */
	long liveHeapMb,
	String gpu,
	String backend,
	int windowWidth,
	int windowHeight,
	long heapMaxMb
) {
	static BenchResult compute(long[] frameNanos, long[] phaseTotals, long gcCount, long gcMs, double allocMbS, double renderAllocMbS, long liveHeapMb, Minecraft mc) {
		int n = frameNanos.length;
		if (n == 0) {
			return new BenchResult(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, new double[phaseTotals.length], gcCount, gcMs, allocMbS, renderAllocMbS, liveHeapMb, "", "", 0, 0, 0);
		}
		long total = 0;
		long jitter = 0;
		for (int i = 0; i < n; i++) {
			total += frameNanos[i];
			if (i > 0) jitter += Math.abs(frameNanos[i] - frameNanos[i - 1]);
		}
		long[] sorted = frameNanos.clone();
		Arrays.sort(sorted);
		double seconds = total / 1e9;

		double[] phases = new double[phaseTotals.length];
		for (int i = 0; i < phases.length; i++) phases[i] = phaseTotals[i] / 1e6 / n;

		String gpu = "unknown";
		String backend = "unknown";
		try {
			var info = RenderSystem.getDevice().getDeviceInfo();
			gpu = info.name();
			backend = info.backendName();
		} catch (RuntimeException ignored) {
			// Device info is best effort.
		}
		var window = mc.getWindow();
		return new BenchResult(
			n,
			seconds,
			n / seconds,
			lowFps(sorted, 0.01),
			lowFps(sorted, 0.001),
			percentileMs(sorted, 0.50),
			percentileMs(sorted, 0.95),
			percentileMs(sorted, 0.99),
			sorted[n - 1] / 1e6,
			n > 1 ? jitter / 1e6 / (n - 1) : 0,
			phases,
			gcCount,
			gcMs,
			allocMbS,
			renderAllocMbS,
			liveHeapMb,
			gpu,
			backend,
			window.getWidth(),
			window.getHeight(),
			Runtime.getRuntime().maxMemory() / (1024 * 1024)
		);
	}

	/** Frame rate of the slowest {@code fraction} of frames ("1% low" for 0.01). */
	static double lowFps(long[] sorted, double fraction) {
		int n = sorted.length;
		int worst = Math.max(1, (int) Math.round(n * fraction));
		long sum = 0;
		for (int i = n - worst; i < n; i++) sum += sorted[i];
		return worst * 1e9 / sum;
	}

	static double percentileMs(long[] sorted, double p) {
		int i = (int) Math.min(sorted.length - 1, Math.round(p * (sorted.length - 1)));
		return sorted[i] / 1e6;
	}

	public String summary() {
		return String.format(
			"%.0f avg, %.0f 1%% low, %.0f 0.1%% low FPS; p99 %.2f ms; jitter %.2f ms; %d GCs (%d ms); alloc %.0f MB/s (render %.0f)",
			avgFps, onePercentLowFps, pointOnePercentLowFps, p99Ms, jitterMs, gcCount, gcMs, allocMbPerSec, renderAllocMbPerSec
		);
	}

	JsonObject toJson() {
		JsonObject o = new JsonObject();
		o.addProperty("exile_bench_version", 1);
		o.addProperty("minecraft", SharedConstants.getCurrentVersion().name());
		o.addProperty("seed", ExileBench.SEED);
		o.addProperty("label", System.getProperty("exile.bench.label", ""));
		o.addProperty("scene", ExileBench.SCENE);
		o.addProperty("frames", frames);
		o.addProperty("seconds", round(seconds));
		o.addProperty("avg_fps", round(avgFps));
		o.addProperty("low_1pct_fps", round(onePercentLowFps));
		o.addProperty("low_0_1pct_fps", round(pointOnePercentLowFps));
		o.addProperty("p50_ms", round(p50Ms));
		o.addProperty("p95_ms", round(p95Ms));
		o.addProperty("p99_ms", round(p99Ms));
		o.addProperty("max_ms", round(maxMs));
		o.addProperty("jitter_ms", round(jitterMs));
		JsonObject phases = new JsonObject();
		FrameStats.Phase[] all = FrameStats.phases();
		for (int i = 0; i < all.length; i++) phases.addProperty(all[i].name().toLowerCase(), round(phaseAvgMs[i]));
		o.add("phase_avg_ms", phases);
		o.addProperty("gc_count", gcCount);
		o.addProperty("gc_ms", gcMs);
		o.addProperty("alloc_mb_per_s", round(allocMbPerSec));
		o.addProperty("render_alloc_mb_per_s", round(renderAllocMbPerSec));
		o.addProperty("live_heap_mb", liveHeapMb);
		JsonObject env = new JsonObject();
		env.addProperty("gpu", gpu);
		env.addProperty("backend", backend);
		env.addProperty("window", windowWidth + "x" + windowHeight);
		env.addProperty("heap_max_mb", heapMaxMb);
		env.addProperty("java", System.getProperty("java.vm.name") + " " + Runtime.version());
		env.addProperty("cpu_threads", Runtime.getRuntime().availableProcessors());
		env.addProperty("jvm_args", String.join(" ", java.lang.management.ManagementFactory.getRuntimeMXBean().getInputArguments().stream()
			.filter(a -> a.startsWith("-XX") || a.startsWith("-Xm")).toList()));
		o.add("environment", env);
		o.add("tuning", ExileBench.tuningJson());
		o.add("mods", ExileBench.modList());
		return o;
	}

	private static double round(double v) {
		return Math.round(v * 100.0) / 100.0;
	}
}
