package dev.exile.client.hud;

import dev.exile.client.ExileClient;
import dev.exile.client.config.ExileConfig;
import dev.exile.client.hud.elements.ArmorElement;
import dev.exile.client.hud.elements.CoordinatesElement;
import dev.exile.client.hud.elements.CpsElement;
import dev.exile.client.hud.elements.EffectsElement;
import dev.exile.client.hud.elements.FpsElement;
import dev.exile.client.hud.elements.FrameBudgetElement;
import dev.exile.client.hud.elements.KeystrokesElement;
import dev.exile.client.hud.elements.SaturationElement;
import dev.exile.client.mixin.ToastManagerAccessor;
import dev.exile.client.ui.HudEditorScreen;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.toasts.Toast;
import net.minecraft.resources.Identifier;

/** Draws every enabled Exile HUD element as one HUD layer. */
public final class ExileHud {
	/** Inner padding when the backdrop is on. */
	public static final int PAD = 3;
	/** Gap kept between elements and the screen edge. */
	public static final int MARGIN = 4;
	/** Width of a standard toast (advancements, tutorial hints, recipes). */
	private static final int TOAST_WIDTH = 160;

	public static final FrameBudgetElement FRAME_BUDGET = new FrameBudgetElement();
	public static final EffectsElement EFFECTS = new EffectsElement();
	public static final List<HudElement> ELEMENTS = List.of(
		new FpsElement(),
		new CoordinatesElement(),
		new ArmorElement(),
		new SaturationElement(),
		EFFECTS,
		new KeystrokesElement(),
		new CpsElement(),
		FRAME_BUDGET
	);

	private static final HudDraw DRAW = new HudDraw();
	/** Laid-out boxes for the in-game HUD (render thread only): no allocation per frame. */
	private static final int[][] BOXES = newBoxes();
	/** Scratch for `layout`: indices of the boxes placed so far. */
	private static final int[] PLACED = new int[ELEMENTS.size() + 1];
	private static final Set<HudElement> FAILED = new HashSet<>();

	private ExileHud() {}

	public static void init() {
		HudElementRegistry.addLast(Identifier.fromNamespaceAndPath("exile", "hud"), (g, delta) -> draw(g));
		HudElementRegistry.attachElementAfter(
			VanillaHudElements.FOOD_BAR, Identifier.fromNamespaceAndPath("exile", "saturation"), (g, delta) -> SaturationOverlay.draw(g)
		);
		// Exile's effects list replaces the vanilla icons when both would show.
		HudElementRegistry.replaceElement(VanillaHudElements.MOB_EFFECTS, vanilla -> (g, delta) -> {
			if (!(EFFECTS.enabled() && ExileConfig.get().features.replaceVanillaEffects)) {
				vanilla.extractRenderState(g, delta);
			}
		});
	}

	public static void tick(Minecraft mc) {
		if (mc.player == null) return;
		for (HudElement e : ELEMENTS) {
			if (!e.enabled() || FAILED.contains(e)) continue;
			try {
				e.tick(mc);
			} catch (RuntimeException ex) {
				fail(e, ex);
			}
		}
	}

	/** A broken element is switched off for the session instead of crashing the game. */
	private static void fail(HudElement e, RuntimeException ex) {
		FAILED.add(e);
		ExileClient.LOG.error("HUD element '{}' failed and is hidden until restart", e.id, ex);
	}

	private static void draw(GuiGraphicsExtractor g) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.player == null || mc.gui.hud.isHidden()) return;
		// The layout editor draws its own preview of every element.
		if (mc.gui.screen() instanceof HudEditorScreen) return;
		DRAW.g = g;
		boolean backdrop = ExileConfig.get().hudBackground;
		layout(mc, g.guiWidth(), g.guiHeight(), backdrop, false, null, BOXES);
		for (int i = 0; i < ELEMENTS.size(); i++) {
			int[] b = BOXES[i];
			if (b[2] == 0) continue;
			HudElement e = ELEMENTS.get(i);
			try {
				drawAt(DRAW, e, b, backdrop, false);
			} catch (RuntimeException ex) {
				fail(e, ex);
			}
		}
	}

	/** One box (x, y, w, h) per element, plus one for the toasts at the end. */
	public static int[][] newBoxes() {
		return new int[ELEMENTS.size() + 1][4];
	}

	/**
	 * Places every visible element at its stored position, except that nothing
	 * may overlap: an element that would cover one placed before it (or the
	 * toasts in the top-right corner) moves just clear of it, down in the top
	 * half of the screen and up in the bottom half. {@code first}, if given, is
	 * placed before the others and never moved, so an element being dragged in
	 * the editor stays under the cursor and the rest make way.
	 *
	 * <p>{@code out[i]} is the box of {@code ELEMENTS.get(i)}; width 0 means not shown.
	 */
	public static void layout(Minecraft mc, int screenW, int screenH, boolean backdrop, boolean preview, HudElement first, int[][] out) {
		int n = ELEMENTS.size();
		int placed = 0;
		int toastBottom = ((ToastManagerAccessor) mc.gui.toastManager()).exile$occupiedSlots().length() * Toast.SLOT_HEIGHT;
		if (toastBottom > 0) {
			int[] t = out[n];
			t[0] = screenW - TOAST_WIDTH;
			t[1] = 0;
			t[2] = TOAST_WIDTH;
			t[3] = toastBottom;
			PLACED[placed++] = n;
		}
		for (int k = first == null ? 0 : -1; k < n; k++) {
			int i = k < 0 ? ELEMENTS.indexOf(first) : k;
			HudElement e = ELEMENTS.get(i);
			if (k >= 0 && e == first) continue;
			int[] b = out[i];
			b[2] = 0;
			if (!e.enabled() || FAILED.contains(e)) continue;
			try {
				if (!preview && !e.hasContent(mc)) continue;
				place(e, screenW, screenH, backdrop, b);
			} catch (RuntimeException ex) {
				fail(e, ex);
				b[2] = 0;
				continue;
			}
			if (e != first) clear(b, out, placed, screenH);
			PLACED[placed++] = i;
		}
	}

	/** Moves {@code b} vertically until it overlaps none of the first {@code placed} boxes, if it fits. */
	private static void clear(int[] b, int[][] boxes, int placed, int screenH) {
		boolean down = b[1] + b[3] / 2 < screenH / 2;
		// Each step clears one box; a few rounds settle any realistic layout.
		for (int round = 0; round < 8; round++) {
			int[] hit = null;
			for (int k = 0; k < placed && hit == null; k++) {
				int[] p = boxes[PLACED[k]];
				if (b[0] < p[0] + p[2] + MARGIN && p[0] < b[0] + b[2] + MARGIN && b[1] < p[1] + p[3] + MARGIN && p[1] < b[1] + b[3] + MARGIN) {
					hit = p;
				}
			}
			if (hit == null) return;
			int y = down ? hit[1] + hit[3] + MARGIN : hit[1] - b[3] - MARGIN;
			// Out of room: leave it where it is rather than push it off screen.
			if (y < MARGIN || y + b[3] > screenH - MARGIN) return;
			b[1] = y;
		}
	}

	/** Screen box (x, y, w, h) of an element from its stored fractional position. */
	public static int[] place(HudElement e, int screenW, int screenH, boolean backdrop, int[] out) {
		int pad = backdrop ? PAD : 0;
		float scale = e.scale();
		int w = Math.round((e.width() + pad * 2) * scale);
		int h = Math.round((e.height() + pad * 2) * scale);
		var slot = e.slot();
		int x = MARGIN + Math.round(Math.clamp(slot.x, 0f, 1f) * Math.max(0, screenW - w - MARGIN * 2));
		int y = MARGIN + Math.round(Math.clamp(slot.y, 0f, 1f) * Math.max(0, screenH - h - MARGIN * 2));
		out[0] = x;
		out[1] = y;
		out[2] = w;
		out[3] = h;
		return out;
	}

	/**
	 * Draws an element into its box (from {@link #layout}), backdrop
	 * included, at the element's own size.
	 */
	public static void drawAt(HudDraw d, HudElement e, int[] box, boolean backdrop, boolean preview) {
		int pad = backdrop ? PAD : 0;
		float scale = e.scale();
		var pose = d.g.pose();
		pose.pushMatrix();
		pose.translate(box[0], box[1]);
		pose.scale(scale, scale);
		try {
			if (backdrop) d.backdrop(0, 0, e.width() + pad * 2, e.height() + pad * 2);
			e.draw(d, pad, pad, preview);
		} finally {
			pose.popMatrix();
		}
	}

	/** Drawing helper for the layout editor. */
	public static HudDraw drawer(GuiGraphicsExtractor g) {
		DRAW.g = g;
		return DRAW;
	}
}
