package dev.exile.client.perf;

import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.util.List;

/**
 * Per-frame timing shared by the FPS counter, Frame Budget and Exile Bench.
 *
 * <p>Mixins call {@link #begin}/{@link #end} around the main phases of a frame
 * and {@link #endFrame} once per frame. Everything is fixed-size arrays and
 * primitive math: nothing here allocates per frame, so measuring never adds
 * garbage-collection pressure of its own. All calls happen on the render
 * thread.
 */
public final class FrameStats {
	private FrameStats() {}

	/** Phases of a frame, in the order they run. */
	public enum Phase {
		GAME("Game logic"),
		WORLD_PREP("World prep"),
		HUD_PREP("HUD prep"),
		WORLD_DRAW("World draw"),
		HUD_DRAW("HUD draw"),
		GPU("GPU & present"),
		IDLE("Frame cap");

		public final String label;

		Phase(String label) {
			this.label = label;
		}
	}

	private static final Phase[] PHASES = Phase.values();
	/** Ring buffer of recent frame times in nanoseconds. */
	private static final int HISTORY = 1024;
	private static final long[] frameNanos = new long[HISTORY];
	/** Reused sort buffer for percentile math (render thread only). */
	private static final long[] SCRATCH = new long[HISTORY];
	private static int head; // index of the next write
	private static int count;

	private static final long[] phaseStart = new long[PHASES.length];
	private static final long[] phaseThisFrame = new long[PHASES.length];
	/** Exponentially smoothed phase times (ns), for a stable live readout. */
	private static final double[] phaseSmoothed = new double[PHASES.length];
	private static double frameSmoothed;
	private static final double SMOOTHING = 0.05;

	private static long lastFrameEnd;
	private static long totalFrames;
	private static long lastFrameNanos;

	// GC load, sampled about once a second.
	private static final List<GarbageCollectorMXBean> GCS = ManagementFactory.getGarbageCollectorMXBeans();
	private static long gcLastSample;
	private static long gcLastTimeMs;
	private static double gcMsPerSecond;

	/** Optional consumer of every frame time (Exile Bench while measuring). */
	private static FrameListener listener;

	@FunctionalInterface
	public interface FrameListener {
		void onFrame(long frameNanos, long[] phaseNanos);
	}

	public static void begin(Phase phase) {
		phaseStart[phase.ordinal()] = System.nanoTime();
	}

	public static void end(Phase phase) {
		int i = phase.ordinal();
		long start = phaseStart[i];
		if (start != 0) {
			phaseThisFrame[i] += System.nanoTime() - start;
			phaseStart[i] = 0;
		}
	}

	/** Called once at the end of every rendered frame. */
	public static void endFrame() {
		long now = System.nanoTime();
		if (lastFrameEnd != 0) {
			long frame = now - lastFrameEnd;
			lastFrameNanos = frame;
			totalFrames++;
			frameNanos[head] = frame;
			head = (head + 1) % HISTORY;
			if (count < HISTORY) count++;

			frameSmoothed += (frame - frameSmoothed) * SMOOTHING;
			for (int i = 0; i < PHASES.length; i++) {
				phaseSmoothed[i] += (phaseThisFrame[i] - phaseSmoothed[i]) * SMOOTHING;
			}
			if (listener != null) listener.onFrame(frame, phaseThisFrame);
		}
		lastFrameEnd = now;
		java.util.Arrays.fill(phaseThisFrame, 0L);
		sampleGc(now);
	}

	private static void sampleGc(long now) {
		if (now - gcLastSample < 1_000_000_000L) return;
		long total = 0;
		for (GarbageCollectorMXBean gc : GCS) {
			long t = gc.getCollectionTime();
			if (t > 0) total += t;
		}
		if (gcLastSample != 0) {
			double seconds = (now - gcLastSample) / 1e9;
			gcMsPerSecond = (total - gcLastTimeMs) / seconds;
		}
		gcLastTimeMs = total;
		gcLastSample = now;
	}

	/** Frames rendered since startup. */
	public static long totalFrames() {
		return totalFrames;
	}

	public static void setListener(FrameListener l) {
		listener = l;
	}

	/** Most recent frame time in ms. */
	public static double lastFrameMs() {
		return lastFrameNanos / 1e6;
	}

	/** Smoothed frame time in ms. */
	public static double smoothedFrameMs() {
		return frameSmoothed / 1e6;
	}

	public static double smoothedPhaseMs(Phase phase) {
		return phaseSmoothed[phase.ordinal()] / 1e6;
	}

	/** Time not attributed to any phase (driver work, input, sound, mods). */
	public static double smoothedOtherMs() {
		double sum = 0;
		for (double p : phaseSmoothed) sum += p;
		return Math.max(0, (frameSmoothed - sum) / 1e6);
	}

	public static double gcMsPerSecond() {
		return gcMsPerSecond;
	}

	/** Average FPS over the last {@code frames} frames. */
	public static double averageFps(int frames) {
		int n = Math.min(frames, count);
		if (n == 0) return 0;
		long sum = 0;
		for (int i = 0; i < n; i++) sum += frameNanos[(head - 1 - i + HISTORY) % HISTORY];
		return sum == 0 ? 0 : n * 1e9 / sum;
	}

	/**
	 * "1% low" FPS over the last {@code frames} frames: the frame rate of the
	 * slowest 1% of frames, which is what players feel as stutter.
	 */
	public static double onePercentLowFps(int frames) {
		int n = Math.min(frames, count);
		if (n < 100) return averageFps(frames);
		long[] copy = SCRATCH;
		for (int i = 0; i < n; i++) copy[i] = frameNanos[(head - 1 - i + HISTORY) % HISTORY];
		java.util.Arrays.sort(copy, 0, n);
		int worst = Math.max(1, n / 100);
		long sum = 0;
		for (int i = n - worst; i < n; i++) sum += copy[i];
		return worst * 1e9 / sum;
	}

	/** Copies the newest {@code out.length} frame times (ns), oldest first. Returns the count copied. */
	public static int recentFrames(long[] out) {
		int n = Math.min(out.length, count);
		for (int i = 0; i < n; i++) out[i] = frameNanos[(head - n + i + HISTORY) % HISTORY];
		return n;
	}

	public static Phase[] phases() {
		return PHASES;
	}
}
