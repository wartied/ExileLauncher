package dev.exile.client.ui;

import com.mojang.blaze3d.platform.cursor.CursorTypes;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

/**
 * Immediate-mode widgets for Exile screens.
 *
 * <p>A screen calls {@link #begin} at the start of every frame, draws its
 * widgets, then {@link #end}. Mouse presses are queued by the screen and
 * claimed by the first widget drawn under them in the next frame, so what is
 * highlighted is always exactly what gets clicked. One look everywhere:
 * flat surfaces, 1px borders, softened corners, the Exile palette.
 */
public final class Ui {
	public static final int ROW = 20;

	private Font font = Minecraft.getInstance().font;
	private GuiGraphicsExtractor g;
	private int mx, my;

	private boolean clickPending;
	private double cx, cy;
	private boolean mouseDown;
	private Object dragOwner;
	private boolean pointer;

	public void begin(GuiGraphicsExtractor graphics, int mouseX, int mouseY) {
		this.g = graphics;
		this.font = Minecraft.getInstance().font;
		this.mx = mouseX;
		this.my = mouseY;
		this.pointer = false;
	}

	/** Drops a click nothing claimed and sets the cursor. */
	public void end() {
		clickPending = false;
		if (pointer) g.requestCursor(CursorTypes.POINTING_HAND);
	}

	public void press(double x, double y) {
		clickPending = true;
		cx = x;
		cy = y;
		mouseDown = true;
	}

	public void release() {
		mouseDown = false;
		dragOwner = null;
	}

	public GuiGraphicsExtractor graphics() {
		return g;
	}

	public int mouseX() {
		return mx;
	}

	public int mouseY() {
		return my;
	}

	// Hit-testing clip: content scrolled out of a clipped area can't be hovered or clicked.
	private int clipX0 = Integer.MIN_VALUE, clipY0 = Integer.MIN_VALUE, clipX1 = Integer.MAX_VALUE, clipY1 = Integer.MAX_VALUE;

	/** Clips drawing and hit-testing to a rectangle (pair with {@link #endClip}). */
	public void clip(int x0, int y0, int x1, int y1) {
		clipX0 = x0;
		clipY0 = y0;
		clipX1 = x1;
		clipY1 = y1;
		g.enableScissor(x0, y0, x1, y1);
	}

	public void endClip() {
		g.disableScissor();
		clipX0 = clipY0 = Integer.MIN_VALUE;
		clipX1 = clipY1 = Integer.MAX_VALUE;
	}

	private boolean inClip(double x, double y) {
		return x >= clipX0 && y >= clipY0 && x < clipX1 && y < clipY1;
	}

	public boolean hover(int x, int y, int w, int h) {
		return mx >= x && my >= y && mx < x + w && my < y + h && inClip(mx, my);
	}

	/** True once for a press inside the rectangle. */
	public boolean click(int x, int y, int w, int h) {
		if (clickPending && cx >= x && cy >= y && cx < x + w && cy < y + h && inClip(cx, cy)) {
			clickPending = false;
			return true;
		}
		return false;
	}

	/** Marks the rectangle as clickable for the cursor; returns hover. */
	public boolean interactive(int x, int y, int w, int h) {
		boolean h0 = hover(x, y, w, h);
		if (h0) pointer = true;
		return h0;
	}

	// ---------------------------------------------------------------- drawing

	/** Filled rectangle with softened (1px notched) corners. */
	public void rect(int x, int y, int w, int h, int color) {
		if (w <= 2 || h <= 2) {
			g.fill(x, y, x + w, y + h, color);
			return;
		}
		g.fill(x + 1, y, x + w - 1, y + h, color);
		g.fill(x, y + 1, x + 1, y + h - 1, color);
		g.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
	}

	/** Surface with a 1px border, both with softened corners. */
	public void panel(int x, int y, int w, int h, int fill, int border) {
		rect(x, y, w, h, border);
		rect(x + 1, y + 1, w - 2, h - 2, fill);
	}

	public void hline(int x0, int x1, int y, int color) {
		g.horizontalLine(x0, x1 - 1, y, color);
	}

	public int width(Component c) {
		return font.width(c);
	}

	public int width(String s) {
		return font.width(Theme.text(s));
	}

	public void text(Component c, int x, int y, int color) {
		g.text(font, c, x, y, color, false);
	}

	/** Draws text vertically centred on {@code centerY}. Returns its width. */
	public int text(String s, int x, int centerY, int color) {
		Component c = Theme.text(s);
		g.text(font, c, x, centerY - 4, color, false);
		return font.width(c);
	}

	public int bold(String s, int x, int centerY, int color) {
		Component c = Theme.bold(s);
		g.text(font, c, x, centerY - 4, color, false);
		return font.width(c);
	}

	public void textRight(String s, int right, int centerY, int color) {
		Component c = Theme.text(s);
		g.text(font, c, right - font.width(c), centerY - 4, color, false);
	}

	public void textCentered(String s, int centerX, int centerY, int color) {
		Component c = Theme.text(s);
		g.text(font, c, centerX - font.width(c) / 2, centerY - 4, color, false);
	}

	/** Word-wrapped text from the top-left corner. Returns the height used. */
	public int paragraph(String s, int x, int y, int maxWidth, int color) {
		var lines = font.split(Theme.text(s), maxWidth);
		for (int i = 0; i < lines.size(); i++) g.text(font, lines.get(i), x, y + i * 10, color, false);
		return lines.size() * 10;
	}

	/** Small capitalised section heading. */
	public void section(String s, int x, int centerY) {
		text(s.toUpperCase(), x, centerY, Theme.TEXT_FAINT);
	}

	/** Shortens {@code s} with an ellipsis to fit {@code maxWidth}. */
	public String fit(String s, int maxWidth) {
		if (width(s) <= maxWidth) return s;
		int lo = 0, hi = s.length();
		while (lo < hi) {
			int mid = (lo + hi + 1) / 2;
			if (width(s.substring(0, mid) + "…") <= maxWidth) lo = mid; else hi = mid - 1;
		}
		return s.substring(0, lo).stripTrailing() + "…";
	}

	// ---------------------------------------------------------------- widgets

	public enum Kind { PRIMARY, NORMAL, DANGER }

	/** A button; returns true when clicked. */
	public boolean button(int x, int y, int w, int h, String label, Kind kind, boolean enabled) {
		boolean hovered = enabled && interactive(x, y, w, h);
		boolean pressed = hovered && mouseDown;
		int fill;
		int border;
		int color;
		if (!enabled) {
			fill = Theme.SURFACE;
			border = Theme.BORDER;
			color = Theme.TEXT_FAINT;
		} else if (kind == Kind.PRIMARY) {
			fill = pressed ? Theme.BUTTON_PRESSED : hovered ? Theme.BUTTON_HOVER : Theme.BUTTON;
			border = fill;
			color = 0xFFFFFFFF;
		} else {
			fill = pressed ? Theme.SURFACE_ACTIVE : hovered ? Theme.SURFACE_HOVER : Theme.SURFACE;
			border = hovered ? Theme.BORDER_STRONG : Theme.BORDER;
			color = kind == Kind.DANGER ? Theme.DANGER : Theme.TEXT;
		}
		panel(x, y, w, h, fill, border);
		Component c = kind == Kind.PRIMARY ? Theme.bold(label) : Theme.text(label);
		g.text(font, c, x + (w - font.width(c)) / 2, y + (h - 7) / 2, color, false);
		return enabled && click(x, y, w, h);
	}

	/** Width {@link #segmented} will take for these labels. */
	public int segmentedWidth(String[] labels) {
		int w = 4;
		for (String l : labels) w += width(l) + 16;
		return w;
	}

	/**
	 * One choice out of a few, as a row of joined segments (like the
	 * launcher's). Returns the index chosen this frame, or {@code active}.
	 */
	public int segmented(int x, int y, int h, String[] labels, int active) {
		panel(x, y, segmentedWidth(labels), h, Theme.SURFACE, Theme.BORDER);
		int chosen = active;
		int sx = x + 2;
		for (int i = 0; i < labels.length; i++) {
			int sw = width(labels[i]) + 16;
			boolean hovered = interactive(sx, y + 2, sw, h - 4);
			if (i == active) rect(sx, y + 2, sw, h - 4, Theme.SURFACE_ACTIVE);
			else if (hovered) rect(sx, y + 2, sw, h - 4, Theme.SURFACE_HOVER);
			textCentered(labels[i], sx + sw / 2, y + h / 2, i == active ? Theme.TEXT : Theme.TEXT_DIM);
			if (i != active && click(sx, y + 2, sw, h - 4)) chosen = i;
			sx += sw;
		}
		return chosen;
	}

	/** On/off switch (22x12). Returns the new state. */
	public boolean toggle(int x, int y, boolean on, boolean enabled) {
		int w = 22, h = 12;
		boolean hovered = enabled && interactive(x - 2, y - 2, w + 4, h + 4);
		int track = !enabled ? Theme.SURFACE_ACTIVE : on ? (hovered ? Theme.BUTTON_HOVER : Theme.BUTTON) : (hovered ? Theme.BORDER_STRONG : Theme.SURFACE_ACTIVE);
		rect(x, y, w, h, track);
		int knob = enabled ? 0xFFFFFFFF : Theme.TEXT_FAINT;
		int kx = on ? x + w - 10 : x + 2;
		rect(kx, y + 2, 8, 8, knob);
		if (enabled && click(x - 2, y - 2, w + 4, h + 4)) return !on;
		return on;
	}

	/** A labelled settings row with a switch on the right. Returns the new state. */
	public boolean toggleRow(int x, int y, int w, String label, String detail, boolean on, boolean enabled) {
		int h = detail == null ? ROW : 28;
		boolean hovered = enabled && hover(x, y, w, h);
		if (hovered) rect(x, y, w, h, Theme.SURFACE_HOVER);
		int labelY = detail == null ? y + h / 2 : y + 9;
		text(label, x + 8, labelY, enabled ? Theme.TEXT : Theme.TEXT_FAINT);
		if (detail != null) text(fit(detail, w - 60), x + 8, y + 20, Theme.TEXT_FAINT);
		boolean next = toggle(x + w - 30, y + (h - 12) / 2, on, enabled);
		// The whole row toggles, not just the switch.
		if (next == on && enabled && interactive(x, y, w, h) && click(x, y, w, h)) next = !on;
		return next;
	}

	/** Horizontal slider. Returns the (stepped) value. */
	public int slider(Object id, int x, int y, int w, int value, int min, int max, int step) {
		int h = 12;
		boolean hovered = interactive(x - 2, y - 2, w + 4, h + 4);
		if (click(x - 2, y - 2, w + 4, h + 4)) dragOwner = id;
		if (dragOwner == id && mouseDown) {
			double t = Math.clamp((mx - x) / (double) w, 0, 1);
			value = min + (int) Math.round(t * (max - min) / step) * step;
		}
		value = Math.clamp(value, min, max);
		float t = (value - min) / (float) (max - min);
		int cy = y + h / 2;
		rect(x, cy - 2, w, 4, Theme.SURFACE_ACTIVE);
		rect(x, cy - 2, Math.max(2, (int) (w * t)), 4, Theme.BUTTON);
		int kx = x + (int) (w * t) - 4;
		rect(kx, cy - 5, 9, 10, hovered || dragOwner == id ? 0xFFFFFFFF : Theme.TEXT);
		return value;
	}

	public boolean dragging(Object id) {
		return dragOwner == id && mouseDown;
	}
}
