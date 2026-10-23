package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.perf.FrameStats;
import dev.exile.client.perf.FrameStats.Phase;
import dev.exile.client.perf.Governor;
import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;

/**
 * Frame Budget: where each frame's time goes. A stacked bar of the frame's
 * phases plus their times, garbage-collector load and the governor's state.
 */
public final class FrameBudgetElement extends HudElement {
	private static final int WIDTH = 150;
	private static final int BAR_H = 5;
	private static final int LINE = 9;
	/** Phase colours, then "Other". */
	public static final int[] COLORS = {
		0xFF8EAEDB, // game logic
		0xFF678FC5, // world prep
		0xFF46679A, // hud prep
		0xFF5FB3A1, // world draw
		0xFF3E8E7E, // hud draw
		0xFFD9A23B, // gpu & present
		0xFF343C4A, // frame cap (idle)
		0xFF97A2B3, // other
	};
	private static final Phase[] PHASES = FrameStats.phases();

	private final double[] ms = new double[PHASES.length + 1];
	private final Component[] rows = new Component[PHASES.length + 1];
	private final Component[] values = new Component[PHASES.length + 1];
	private Component header = Theme.bold("Frame budget");
	private Component gc = Labels.dim("GC");
	private Component governor;
	private double frameMs;

	public FrameBudgetElement() {
		super("frame_budget", "Frame Budget", "Where each frame's time goes, live", false, 0f, 0f);
		// Drawable before the first tick: it can be switched on, or previewed
		// in the layout editor, between two ticks.
		for (int i = 0; i < rows.length; i++) {
			rows[i] = Labels.dim(i < PHASES.length ? PHASES[i].label : "Other");
			values[i] = Theme.bold("0.00");
		}
	}

	@Override
	public void tick(Minecraft mc) {
		frameMs = FrameStats.smoothedFrameMs();
		for (int i = 0; i < PHASES.length; i++) ms[i] = FrameStats.smoothedPhaseMs(PHASES[i]);
		ms[PHASES.length] = FrameStats.smoothedOtherMs();
		double fps = frameMs > 0 ? 1000 / frameMs : 0;
		header = Labels.line().append(Theme.bold("Frame budget  ")).append(Labels.dim(String.format("%.2f ms  ·  %.0f fps", frameMs, fps)));
		for (int i = 0; i < rows.length; i++) {
			String label = i < PHASES.length ? PHASES[i].label : "Other";
			rows[i] = Labels.dim(label);
			values[i] = Theme.bold(String.format("%.2f", ms[i]));
		}
		gc = Labels.line().append(Labels.dim("Garbage collection  ")).append(Theme.bold(String.format("%.0f ms/s", FrameStats.gcMsPerSecond())));
		String status = Governor.status();
		governor = status == null ? null : Labels.colored(status, Theme.ACCENT_LIGHT);
	}

	@Override
	public int width() {
		return WIDTH;
	}

	@Override
	public int height() {
		return LINE + 4 + BAR_H + 4 + rows.length * LINE + LINE + (governor != null ? LINE : 0);
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		d.text(header, x, y, Theme.TEXT);
		int by = y + LINE + 3;
		// Stacked bar: each phase's share of the frame.
		double total = Math.max(frameMs, 0.001);
		d.fill(x, by, WIDTH, BAR_H, Theme.alpha(Theme.SURFACE_ACTIVE, 0.9f));
		int cx = x;
		for (int i = 0; i < ms.length; i++) {
			int w = (int) Math.round(WIDTH * Math.min(1.0, ms[i] / total));
			if (w <= 0) continue;
			w = Math.min(w, x + WIDTH - cx);
			d.fill(cx, by, w, BAR_H, COLORS[i]);
			cx += w;
		}
		int ry = by + BAR_H + 5;
		for (int i = 0; i < rows.length; i++) {
			d.fill(x, ry + 1, 5, 5, COLORS[i]);
			d.text(rows[i], x + 9, ry, Theme.TEXT);
			d.text(values[i], x + WIDTH - d.width(values[i]), ry, ms[i] >= 0.005 ? Theme.TEXT : Theme.TEXT_FAINT);
			ry += LINE;
		}
		d.text(gc, x, ry, Theme.TEXT);
		if (governor != null) d.text(governor, x, ry + LINE, Theme.TEXT);
	}
}
