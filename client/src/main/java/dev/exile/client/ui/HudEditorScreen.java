package dev.exile.client.ui;

import com.mojang.blaze3d.platform.InputConstants;
import dev.exile.client.config.ExileConfig;
import dev.exile.client.hud.ExileHud;
import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Ui.Kind;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

/**
 * HUD layout editor: drag elements to move them. They snap to the screen
 * edges and centre lines; positions are stored as fractions of the screen so
 * the layout survives window and GUI-scale changes. Click an element to
 * select it, then resize it (or scroll over it) and pick what it shows.
 */
public final class HudEditorScreen extends Screen {
	private static final int SNAP = 4;
	private static final float[] SIZES = {0.5f, 0.75f, 1f, 1.25f, 1.5f, 1.75f, 2f};
	private final Screen parent;
	private final Ui ui = new Ui();
	private final int[] box = new int[4];
	/** Where each element was drawn last frame (see ExileHud.layout), for hit-testing. */
	private final int[][] boxes = ExileHud.newBoxes();
	private HudElement dragging;
	/** The element the toolbar's size and mode controls apply to. */
	private HudElement selected;
	/** The toolbar's area last frame: clicks there don't change the selection. */
	private final int[] toolbar = new int[4];
	private int grabX, grabY;
	private boolean mouseDown;
	private boolean snappedX, snappedY;

	public HudEditorScreen(Screen parent) {
		super(Component.literal("Edit HUD layout"));
		this.parent = parent;
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		if (event.button() != InputConstants.MOUSE_BUTTON_LEFT) return true;
		ui.press(event.x(), event.y());
		mouseDown = true;
		if (inside(toolbar, event.x(), event.y())) return true;
		selected = null;
		for (int i = 0; i < ExileHud.ELEMENTS.size(); i++) {
			int[] b = boxes[i];
			if (inside(b, event.x(), event.y())) {
				dragging = ExileHud.ELEMENTS.get(i);
				selected = dragging;
				// Grab it where it was shown, which may differ from its stored
				// position if it was making way for another element.
				grabX = (int) event.x() - b[0];
				grabY = (int) event.y() - b[1];
				move(dragging, b[0], b[1]);
				break;
			}
		}
		return true;
	}

	/** Scrolling over an element resizes it. */
	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		for (int i = 0; i < ExileHud.ELEMENTS.size(); i++) {
			if (inside(boxes[i], x, y) && scrollY != 0) {
				selected = ExileHud.ELEMENTS.get(i);
				resize(selected, scrollY > 0 ? 1 : -1);
				return true;
			}
		}
		return true;
	}

	private static boolean inside(int[] b, double x, double y) {
		return b[2] > 0 && x >= b[0] && y >= b[1] && x < b[0] + b[2] && y < b[1] + b[3];
	}

	/** One step bigger (+1) or smaller (-1). */
	private static void resize(HudElement e, int direction) {
		var slot = e.slot();
		int nearest = 0;
		for (int i = 1; i < SIZES.length; i++) {
			if (Math.abs(SIZES[i] - slot.scale) < Math.abs(SIZES[nearest] - slot.scale)) nearest = i;
		}
		slot.scale = SIZES[Math.clamp(nearest + direction, 0, SIZES.length - 1)];
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		ui.release();
		mouseDown = false;
		dragging = null;
		snappedX = snappedY = false;
		return true;
	}

	@Override
	public void onClose() {
		minecraft.gui.setScreen(parent);
	}

	/** Called whenever this screen goes away, including when the game quits with it open. */
	@Override
	public void removed() {
		ExileConfig.get().save();
	}

	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		g.fill(0, 0, width, height, 0x40000000);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		ui.begin(g, mouseX, mouseY);
		if (dragging != null && mouseDown) move(dragging, mouseX - grabX, mouseY - grabY);

		if (snappedX) g.fill(width / 2, 0, width / 2 + 1, height, Theme.alpha(Theme.ACCENT, 0.6f));
		if (snappedY) g.fill(0, height / 2, width, height / 2 + 1, Theme.alpha(Theme.ACCENT, 0.6f));

		boolean backdrop = ExileConfig.get().hudBackground;
		HudDraw d = ExileHud.drawer(g);
		if (minecraft.player != null) {
			for (HudElement e : ExileHud.ELEMENTS) {
				if (e.enabled()) e.tick(minecraft);
			}
		}
		ExileHud.layout(minecraft, width, height, backdrop, true, dragging, boxes);
		for (int i = 0; i < ExileHud.ELEMENTS.size(); i++) {
			HudElement e = ExileHud.ELEMENTS.get(i);
			int x = boxes[i][0], y = boxes[i][1], w = boxes[i][2], h = boxes[i][3];
			if (w == 0) continue;
			ExileHud.drawAt(d, e, boxes[i], backdrop, true);
			boolean active = e == selected || e == dragging || (dragging == null && ui.interactive(x, y, w, h));
			outline(g, x - 1, y - 1, w + 2, h + 2, active ? Theme.ACCENT : Theme.alpha(Theme.TEXT, 0.25f));
		}
		toolbar(selected != null && selected.enabled() ? selected : null);
		ui.end();
	}

	/**
	 * Centred near the bottom. With nothing selected: a hint and Done. With an
	 * element selected: its name, size (- 100% +), what it shows, and Done.
	 */
	private void toolbar(HudElement e) {
		int th = 30, gap = 8;
		String[] modes = e == null ? null : e.modes().stream().map(m -> m.label).toArray(String[]::new);
		int content;
		if (e == null) {
			content = ui.width("Click an element to resize it, or drag it to move");
		} else {
			content = ui.width(e.name) + gap * 2 + 18 + 40 + 18;
			if (modes.length > 0) content += gap * 2 + ui.segmentedWidth(modes);
		}
		int tw = 12 + content + gap * 2 + 64 + 12;
		int tx = (width - tw) / 2, ty = height - th - 16;
		toolbar[0] = tx;
		toolbar[1] = ty;
		toolbar[2] = tw;
		toolbar[3] = th;
		ui.panel(tx, ty, tw, th, Theme.BG, Theme.BORDER_STRONG);
		int x = tx + 12, cy = ty + th / 2;
		if (e == null) {
			ui.text("Click an element to resize it, or drag it to move", x, cy, Theme.TEXT_DIM);
		} else {
			x += ui.bold(e.name, x, cy, Theme.TEXT) + gap * 2;
			if (ui.button(x, ty + 6, 18, 18, "-", Kind.NORMAL, e.scale() > SIZES[0])) resize(e, -1);
			x += 18;
			ui.textCentered(Math.round(e.scale() * 100) + "%", x + 20, cy, Theme.TEXT);
			x += 40;
			if (ui.button(x, ty + 6, 18, 18, "+", Kind.NORMAL, e.scale() < SIZES[SIZES.length - 1])) resize(e, 1);
			x += 18;
			if (modes.length > 0) {
				x += gap * 2;
				int active = e.modes().indexOf(e.mode());
				int chosen = ui.segmented(x, ty + 6, 18, modes, active);
				if (chosen != active) e.setMode(e.modes().get(chosen));
			}
		}
		if (ui.button(tx + tw - 76, ty + 6, 64, 18, "Done", Kind.PRIMARY, true)) onClose();
	}

	private void move(HudElement e, int x, int y) {
		boolean backdrop = ExileConfig.get().hudBackground;
		ExileHud.place(e, width, height, backdrop, box);
		int w = box[2], h = box[3];
		int m = ExileHud.MARGIN;
		// Free space for the element's top-left corner, inside the screen margin.
		int maxX = Math.max(1, width - w - m * 2), maxY = Math.max(1, height - h - m * 2);
		x = Math.clamp(x - m, 0, maxX);
		y = Math.clamp(y - m, 0, maxY);
		// Snap to edges and to the centre lines.
		if (x < SNAP) x = 0;
		if (maxX - x < SNAP) x = maxX;
		if (y < SNAP) y = 0;
		if (maxY - y < SNAP) y = maxY;
		snappedX = Math.abs(m + x + w / 2 - width / 2) < SNAP;
		if (snappedX) x = width / 2 - w / 2 - m;
		snappedY = Math.abs(m + y + h / 2 - height / 2) < SNAP;
		if (snappedY) y = height / 2 - h / 2 - m;
		var slot = e.slot();
		slot.x = Math.clamp(x / (float) maxX, 0f, 1f);
		slot.y = Math.clamp(y / (float) maxY, 0f, 1f);
	}

	private static void outline(GuiGraphicsExtractor g, int x, int y, int w, int h, int color) {
		g.fill(x, y, x + w, y + 1, color);
		g.fill(x, y + h - 1, x + w, y + h, color);
		g.fill(x, y + 1, x + 1, y + h - 1, color);
		g.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}
}
