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
 * the layout survives window and GUI-scale changes.
 */
public final class HudEditorScreen extends Screen {
	private static final int SNAP = 4;
	private final Screen parent;
	private final Ui ui = new Ui();
	private final int[] box = new int[4];
	/** Where each element was drawn last frame (see ExileHud.layout), for hit-testing. */
	private final int[][] boxes = ExileHud.newBoxes();
	private HudElement dragging;
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
		for (int i = 0; i < ExileHud.ELEMENTS.size(); i++) {
			int[] b = boxes[i];
			if (b[2] > 0 && event.x() >= b[0] && event.y() >= b[1] && event.x() < b[0] + b[2] && event.y() < b[1] + b[3]) {
				dragging = ExileHud.ELEMENTS.get(i);
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
			if (backdrop) d.backdrop(x, y, w, h);
			int pad = backdrop ? ExileHud.PAD : 0;
			e.draw(d, x + pad, y + pad, true);
			boolean active = e == dragging || (dragging == null && ui.interactive(x, y, w, h));
			outline(g, x - 1, y - 1, w + 2, h + 2, active ? Theme.ACCENT : Theme.alpha(Theme.TEXT, 0.25f));
		}

		// Toolbar: hint and Done, centred near the bottom.
		int tw = 300, th = 30;
		int tx = (width - tw) / 2, ty = height - th - 16;
		ui.panel(tx, ty, tw, th, Theme.BG, Theme.BORDER_STRONG);
		ui.text("Drag elements to move them", tx + 12, ty + th / 2, Theme.TEXT_DIM);
		if (ui.button(tx + tw - 72, ty + 6, 64, 18, "Done", Kind.PRIMARY, true)) onClose();
		ui.end();
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
