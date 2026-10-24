package dev.exile.client.ui;

import com.mojang.blaze3d.Blaze3D;
import com.mojang.blaze3d.platform.InputConstants;
import dev.exile.client.bench.BenchHistory;
import dev.exile.client.bench.BenchResult;
import dev.exile.client.bench.ExileBench;
import dev.exile.client.config.ExileConfig;
import dev.exile.client.feature.Keybinds;
import dev.exile.client.hud.ExileHud;
import dev.exile.client.hud.HudElement;
import dev.exile.client.perf.BackgroundThrottle;
import dev.exile.client.perf.Tuning;
import dev.exile.client.ui.Ui.Kind;
import java.util.List;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

/**
 * The Exile menu: HUD, features, performance, mods and benchmark in one
 * minimal panel. Opened from the pause menu or with Right Shift.
 */
public final class ExileMenuScreen extends Screen {
	public enum Tab {
		HUD("HUD", "Choose what is on screen and where."),
		FEATURES("Features", "Camera, visibility, movement and interface. All client-side."),
		PERFORMANCE("Performance", "Every optimisation here was measured before it shipped."),
		MODS("Mods", ""),
		BENCH("Bench", "A repeatable benchmark: one warm-up lap, one measured lap.");

		final String title;
		final String subtitle;

		Tab(String title, String subtitle) {
			this.title = title;
			this.subtitle = subtitle;
		}
	}

	private static final int SIDE = 128;
	private final Screen parent;
	private final Ui ui = new Ui();
	private Tab tab;
	private double scroll;
	/** The key binding waiting for a key press, if any (see keyRow). */
	private KeyMapping capturing;
	private int contentHeight;
	private List<ModEntries.Entry> mods;
	private List<BenchHistory.Row> history;
	private boolean confirmBench;
	private BenchResult lastResult;
	private String notice;

	public ExileMenuScreen(Screen parent) {
		this(parent, Tab.HUD);
	}

	public ExileMenuScreen(Screen parent, Tab tab) {
		super(Component.literal("Exile"));
		this.parent = parent;
		select(tab);
	}

	/** Opened by the Mod Menu API shim (ModMenuApi.createModsScreen). */
	public static Screen modsTab(Screen previous) {
		return new ExileMenuScreen(previous, Tab.MODS);
	}

	private void select(Tab t) {
		tab = t;
		scroll = 0;
		confirmBench = false;
		notice = null;
		if (t == Tab.MODS && mods == null) mods = ModEntries.load();
		if (t == Tab.BENCH) history = BenchHistory.recent(6);
	}

	// ---------------------------------------------------------------- input

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		// While rebinding, any button but the left one becomes the key.
		if (capturing != null && event.button() != InputConstants.MOUSE_BUTTON_LEFT) {
			bind(InputConstants.Type.MOUSE.getOrCreate(event.button()));
			return true;
		}
		if (event.button() == InputConstants.MOUSE_BUTTON_LEFT) ui.press(event.x(), event.y());
		return true;
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		if (capturing != null) {
			if (event.key() == InputConstants.KEY_ESCAPE) capturing = null;
			else bind(InputConstants.getKey(event));
			return true;
		}
		return super.keyPressed(event);
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		ui.release();
		return true;
	}

	@Override
	public boolean mouseScrolled(double x, double y, double scrollX, double scrollY) {
		scroll -= scrollY * 20;
		return true;
	}

	@Override
	public void onClose() {
		minecraft.gui.setScreen(parent);
	}

	/** Called whenever this screen goes away, including when the game quits with it open. */
	@Override
	public void removed() {
		capturing = null;
		ExileConfig.get().save();
		minecraft.options.save();
	}

	// ---------------------------------------------------------------- drawing

	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		// Flat dim instead of vanilla's blur: cheaper, and the game stays readable behind.
		if (minecraft.level == null) extractPanorama(g, a);
		g.fill(0, 0, width, height, Theme.BACKDROP);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float a) {
		ui.begin(g, mouseX, mouseY);
		int pw = Math.min(width - 24, 600);
		int ph = Math.min(height - 24, 360);
		int px = (width - pw) / 2;
		int py = (height - ph) / 2;
		ui.panel(px, py, pw, ph, Theme.BG, Theme.BORDER_STRONG);

		// Sidebar
		g.fill(px + 1, py + 1, px + SIDE, py + ph - 1, Theme.SIDEBAR);
		g.fill(px + SIDE, py + 1, px + SIDE + 1, py + ph - 1, Theme.BORDER);
		brand(px + 14, py + 20);
		int ty = py + 40;
		for (Tab t : Tab.values()) {
			int tx = px + 8, tw = SIDE - 16, th = 20;
			boolean selected = t == tab;
			boolean hovered = !selected && ui.interactive(tx, ty, tw, th);
			if (selected) {
				ui.rect(tx, ty, tw, th, Theme.SURFACE_ACTIVE);
				g.fill(tx, ty + 5, tx + 2, ty + th - 5, Theme.ACCENT);
			} else if (hovered) {
				ui.rect(tx, ty, tw, th, Theme.SURFACE_HOVER);
			}
			if (selected) ui.bold(t.title, tx + 10, ty + th / 2, Theme.TEXT);
			else ui.text(t.title, tx + 10, ty + th / 2, hovered ? Theme.TEXT : Theme.TEXT_DIM);
			if (!selected && ui.click(tx, ty, tw, th)) select(t);
			ty += th + 2;
		}
		String version = FabricLoader.getInstance().getModContainer("exile").map(m -> m.getMetadata().getVersion().getFriendlyString()).orElse("");
		ui.text("Exile Client " + version, px + 14, py + ph - 14, Theme.TEXT_FAINT);

		// Content
		int cx = px + SIDE + 20;
		int cw = pw - SIDE - 40;
		int top = py + 18;
		ui.bold(tab.title, cx, top + 4, Theme.TEXT);
		String sub = tab == Tab.MODS ? (mods == null ? "" : mods.size() + " mods loaded") : tab.subtitle;
		ui.text(ui.fit(sub, cw), cx, top + 18, Theme.TEXT_DIM);

		int bodyTop = top + 34;
		int bodyBottom = py + ph - 12;
		int maxScroll = Math.max(0, contentHeight - (bodyBottom - bodyTop));
		scroll = Math.clamp(scroll, 0, maxScroll);
		ui.clip(cx - 4, bodyTop, cx + cw + 4, bodyBottom);
		int y = bodyTop - (int) scroll;
		int end = switch (tab) {
			case HUD -> hudTab(cx, y, cw);
			case FEATURES -> featuresTab(cx, y, cw);
			case PERFORMANCE -> performanceTab(cx, y, cw);
			case MODS -> modsTab(cx, y, cw, bodyTop, bodyBottom);
			case BENCH -> benchTab(cx, y, cw);
		};
		ui.endClip();
		contentHeight = end - y;
		if (maxScroll > 0) scrollbar(cx + cw + 6, bodyTop, bodyBottom - bodyTop, maxScroll);
		ui.end();
	}

	/** Letter-spaced wordmark, like the launcher's. */
	private void brand(int x, int centerY) {
		for (char c : "EXILE".toCharArray()) x += ui.bold(String.valueOf(c), x, centerY, Theme.TEXT) + 3;
	}

	private void scrollbar(int x, int y, int h, int maxScroll) {
		int thumb = Math.max(16, h * h / (h + maxScroll));
		int ty = y + (int) ((h - thumb) * (scroll / maxScroll));
		ui.graphics().fill(x, ty, x + 2, ty + thumb, Theme.BORDER_STRONG);
	}

	// ---------------------------------------------------------------- tabs

	private int hudTab(int x, int y, int w) {
		var cfg = ExileConfig.get();
		// Layout row: label on the left, the action on the right.
		boolean inWorld = minecraft.player != null;
		ui.text("Layout", x + 8, y + 9, Theme.TEXT);
		ui.text(inWorld ? "Drag elements to where you want them" : "Join a world to edit the layout", x + 8, y + 20, Theme.TEXT_FAINT);
		if (ui.button(x + w - 70, y + 5, 62, 18, "Edit", Kind.PRIMARY, inWorld)) minecraft.gui.setScreen(new HudEditorScreen(this));
		y += 32;
		boolean bg = ui.toggleRow(x, y, w, "Backdrop", "A subtle dark box behind each element", cfg.hudBackground, true);
		if (bg != cfg.hudBackground) cfg.hudBackground = bg;
		y += 32;
		ui.section("Elements", x + 8, y + 4);
		y += 12;
		for (HudElement e : ExileHud.ELEMENTS) {
			var slot = e.slot();
			boolean on = ui.toggleRow(x, y, w, e.name, e.description, slot.enabled, true);
			if (on != slot.enabled) slot.enabled = on;
			y += 30;
		}
		return y;
	}

	private int featuresTab(int x, int y, int w) {
		var f = ExileConfig.get().features;
		var o = minecraft.options;

		ui.section("Camera", x + 8, y + 4);
		y += 12;
		f.freelook = ui.toggleRow(x, y, w, "Free look", "Hold the key to look around without turning", f.freelook, true);
		y += 30;
		if (f.freelook) y = keyRow(x, y, w, "Free look key", Keybinds.freelook);
		f.zoom = ui.toggleRow(x, y, w, "Zoom", "Hold the key to magnify the view", f.zoom, true);
		y += 30;
		if (f.zoom) {
			y = keyRow(x, y, w, "Zoom key", Keybinds.zoom);
			y = sliderRow(x, y, w, "Zoom amount", "zoom", f.zoomLevel, 2, 10, 1, "x", v -> f.zoomLevel = v);
			f.zoomToggle = ui.toggleRow(x, y, w, "Toggle zoom", "Press once to zoom and again to stop, instead of holding", f.zoomToggle, true);
			y += 30;
			f.zoomSmooth = ui.toggleRow(x, y, w, "Smooth zoom", "Ease in and out instead of jumping", f.zoomSmooth, true);
			y += 30;
			f.zoomScroll = ui.toggleRow(x, y, w, "Scroll to adjust", "Mouse wheel zooms further in or out while zoomed", f.zoomScroll, true);
			y += 30;
			f.zoomCinematic = ui.toggleRow(x, y, w, "Cinematic camera", "Smooth, drifting camera movement while zoomed", f.zoomCinematic, true);
			y += 30;
		}

		y += 6;
		ui.section("Visibility", x + 8, y + 4);
		y += 12;
		f.fullbright = ui.toggleRow(x, y, w, "Fullbright", "See everything as if in full light", f.fullbright, true);
		y += 30;
		y = keyRow(x, y, w, "Fullbright key", Keybinds.fullbright);
		f.clearWater = ui.toggleRow(x, y, w, "Clear water", "See as far underwater as on land", f.clearWater, true);
		y += 30;
		f.clearLava = ui.toggleRow(x, y, w, "Clear lava", "See through lava when you're in it", f.clearLava, true);
		y += 30;
		f.lowFire = ui.toggleRow(x, y, w, "Low fire", "Flames on screen sit lower when you're burning", f.lowFire, true);
		y += 30;
		f.lowShield = ui.toggleRow(x, y, w, "Low shield", "Your shield sits lower, so it blocks less of the view", f.lowShield, true);
		y += 30;

		y += 6;
		ui.section("Movement", x + 8, y + 4);
		y += 12;
		boolean sprint = ui.toggleRow(x, y, w, "Toggle sprint", "Press sprint once to keep sprinting", o.toggleSprint().get(), true);
		if (sprint != o.toggleSprint().get()) o.toggleSprint().set(sprint);
		y += 30;
		boolean sneak = ui.toggleRow(x, y, w, "Toggle sneak", "Press sneak once to keep sneaking", o.toggleCrouch().get(), true);
		if (sneak != o.toggleCrouch().get()) o.toggleCrouch().set(sneak);
		y += 30;

		y += 6;
		ui.section("Armour", x + 8, y + 4);
		y += 12;
		f.slimArmour = ui.toggleRow(x, y, w, "Slim armour", "Armour sits closer to the body, on every player", f.slimArmour, true);
		y += 30;
		f.invisibleArmour = ui.toggleRow(
			x, y, w, "Invisible armour", "Hide your own armour so your skin shows. Only you see this", f.invisibleArmour, true
		);
		y += 30;

		y += 6;
		ui.section("Interface", x + 8, y + 4);
		y += 12;
		ui.text("Menu theme", x + 8, y + 9, Theme.TEXT);
		ui.text(ui.fit("Darker vanilla menus", w - 200), x + 8, y + 20, Theme.TEXT_FAINT);
		String[] themes = {"Vanilla", "Dark", "Exile"};
		int theme = MenuTheme.mode().ordinal();
		int picked = ui.segmented(x + w - 8 - ui.segmentedWidth(themes), y + 5, 18, themes, theme);
		if (picked != theme) MenuTheme.set(MenuTheme.Mode.values()[picked]);
		y += 32;
		f.saturationOverlay = ui.toggleRow(
			x, y, w, "Saturation on hunger bar", "Gold outlines show how much saturation is left", f.saturationOverlay, true
		);
		y += 30;
		f.replaceVanillaEffects = ui.toggleRow(
			x, y, w, "Replace effect icons", "Hide vanilla's effect icons while Exile's list is on", f.replaceVanillaEffects, true
		);
		y += 30;
		ui.text("Exile's keys are also under Options, Controls, Exile.", x + 8, y + 8, Theme.TEXT_FAINT);
		return y + 18;
	}

	/**
	 * A key binding with its key on a button. Click it, then press a key (or a
	 * side or middle mouse button) to rebind; Esc or clicking again cancels.
	 */
	private int keyRow(int x, int y, int w, String label, KeyMapping key) {
		boolean listening = capturing == key;
		ui.text(label, x + 8, y + 10, Theme.TEXT_DIM);
		String name = listening ? "Press a key" : key.getTranslatedKeyMessage().getString();
		if (ui.button(x + w - 98, y + 1, 90, 18, ui.fit(name, 82), listening ? Kind.PRIMARY : Kind.NORMAL, true)) {
			capturing = listening ? null : key;
		}
		return y + 24;
	}

	private void bind(InputConstants.Key key) {
		capturing.setKey(key);
		KeyMapping.resetMapping();
		minecraft.options.save();
		capturing = null;
	}

	private int performanceTab(int x, int y, int w) {
		var p = ExileConfig.get().performance;
		ui.section("Frame rate", x + 8, y + 4);
		y += 12;
		// VSync caps everything below at the display's refresh rate, which is
		// easy to mistake for one of these settings holding the frame rate.
		if (minecraft.options.enableVsync().get()) {
			var monitor = minecraft.getWindow().findBestMonitor();
			int hz = monitor == null ? 0 : Math.round(monitor.currentMode().getRefreshRate());
			String cap = hz > 0 ? hz + " Hz" : "refresh rate";
			y += ui.paragraph("VSync is on, so FPS stops at your display's " + cap + ". Turn it off in Video Settings to go higher.", x + 8, y + 4, w - 16, Theme.TEXT_DIM) + 8;
		}
		boolean bt = ui.toggleRow(
			x, y, w, BackgroundThrottle.SWITCH.title, "Limit FPS in another window (about 70% less GPU power in testing)",
			BackgroundThrottle.SWITCH.enabled(), !BackgroundThrottle.SWITCH.forced()
		);
		if (bt != BackgroundThrottle.SWITCH.enabled()) BackgroundThrottle.SWITCH.set(bt);
		y += 30;
		if (bt) {
			y = sliderRow(x, y, w, "Background FPS", "bg", p.backgroundFps, 5, 120, 5, " fps", v -> p.backgroundFps = v);
		}
		var fb = ExileHud.FRAME_BUDGET.slot();
		fb.enabled = ui.toggleRow(x, y, w, "Frame Budget overlay", "Live breakdown of where each frame's time goes", fb.enabled, true);
		y += 30;
		p.governor = ui.toggleRow(
			x, y, w, "Hold a target frame rate", "Trims the least noticeable settings first when FPS drops", p.governor, true
		);
		y += 30;
		if (p.governor) {
			y = sliderRow(x, y, w, "Target FPS", "target", p.governorTargetFps, 30, 240, 10, " fps", v -> p.governorTargetFps = v);
		}

		List<Tuning.Switch> opts = Tuning.all().stream().filter(s -> s != BackgroundThrottle.SWITCH).toList();
		if (!opts.isEmpty()) {
			y += 6;
			ui.section("Optimisations", x + 8, y + 4);
			y += 12;
			for (Tuning.Switch s : opts) {
				String detail = s.forced() ? s.description + " (set by launch options)" : s.description;
				boolean on = ui.toggleRow(x, y, w, s.title, detail, s.enabled(), !s.forced());
				if (on != s.enabled()) s.set(on);
				y += 30;
			}
		}
		return y;
	}

	private int sliderRow(int x, int y, int w, String label, Object id, int value, int min, int max, int step, String unit, java.util.function.IntConsumer set) {
		ui.text(label, x + 8, y + 10, Theme.TEXT_DIM);
		ui.textRight(value + unit, x + w - 8, y + 10, Theme.TEXT);
		int sx = x + 110;
		int next = ui.slider(id, sx, y + 4, w - 110 - 60, value, min, max, step);
		if (next != value) set.accept(next);
		return y + 24;
	}

	private int modsTab(int x, int y, int w, int visibleTop, int visibleBottom) {
		for (ModEntries.Entry m : mods) {
			int h = 34;
			// Skip drawing rows outside the visible area: long mod lists stay cheap.
			if (y + h >= visibleTop && y <= visibleBottom) {
				if (ui.hover(x, y, w, h)) ui.rect(x, y, w, h, Theme.SURFACE_HOVER);
				boolean configurable = m.config() != null || m.id().equals("exile");
				int textW = w - 16 - (configurable ? 84 : 0);
				int nameW = ui.bold(ui.fit(m.name(), textW - 60), x + 8, y + 11, Theme.TEXT);
				ui.text(ui.fit(m.version(), textW - nameW - 8), x + 8 + nameW + 6, y + 11, Theme.TEXT_FAINT);
				ui.text(ui.fit(m.description().replace('\n', ' '), textW), x + 8, y + 24, Theme.TEXT_DIM);
				if (configurable && ui.button(x + w - 84, y + 8, 76, 18, "Configure", Kind.NORMAL, true)) {
					openConfig(m);
				}
			}
			y += h + 2;
		}
		if (notice != null) {
			ui.text(notice, x + 8, y + 8, Theme.DANGER);
			y += 18;
		}
		return y;
	}

	private void openConfig(ModEntries.Entry m) {
		if (m.id().equals("exile")) {
			select(Tab.HUD);
			return;
		}
		try {
			Screen screen = m.config().create(this);
			if (screen != null) minecraft.gui.setScreen(screen);
			else notice = m.name() + " has nothing to configure.";
		} catch (Throwable t) {
			notice = "Couldn't open " + m.name() + "'s settings.";
			dev.exile.client.ExileClient.LOG.warn("Config screen of {} failed", m.id(), t);
		}
	}

	private int benchTab(int x, int y, int w) {
		y += ui.paragraph(
			"Flies a fixed path through a fixed world with fixed settings, so results compare fairly between settings, "
				+ "mods and computers. It takes about a minute and your settings are put back afterwards.",
			x + 8, y, w - 16, Theme.TEXT_DIM
		);
		y += 10;
		boolean inWorld = minecraft.level != null;
		String label = inWorld && confirmBench ? "Leave world and run" : "Run benchmark";
		if (ui.button(x + 8, y, 150, 22, label, Kind.PRIMARY, !ExileBench.running())) {
			if (inWorld && !confirmBench) {
				confirmBench = true;
			} else {
				// When done, leave the bench world and show the results; closing them returns to the title screen.
				ExileBench.start(false, result -> minecraft.disconnect(withResult(result), false));
			}
		}
		if (inWorld) {
			ui.text(confirmBench ? "You'll leave this world first." : "Runs in its own world.", x + 168, y + 11, confirmBench ? Theme.WARN : Theme.TEXT_FAINT);
		}
		y += 34;

		if (lastResult != null) {
			ui.section("Latest run", x + 8, y + 4);
			y += 14;
			y = bigNumbers(x + 8, y, lastResult.avgFps(), lastResult.onePercentLowFps(), lastResult.pointOnePercentLowFps());
			y += 8;
		}

		ui.section("Recent results", x + 8, y + 4);
		if (ui.button(x + w - 92, y - 4, 84, 16, "Open folder", Kind.NORMAL, true)) Blaze3D.openPath(BenchHistory.dir());
		y += 16;
		if (history == null || history.isEmpty()) {
			ui.text("No results yet.", x + 8, y + 8, Theme.TEXT_FAINT);
			return y + 18;
		}
		int c1 = x + 8, c2 = x + w - 190, c3 = x + w - 120, c4 = x + w - 50;
		ui.text("When", c1, y + 6, Theme.TEXT_FAINT);
		ui.textRight("Average", c2 + 40, y + 6, Theme.TEXT_FAINT);
		ui.textRight("1% low", c3 + 40, y + 6, Theme.TEXT_FAINT);
		ui.textRight("0.1% low", c4 + 40, y + 6, Theme.TEXT_FAINT);
		y += 14;
		for (BenchHistory.Row r : history) {
			ui.graphics().fill(x + 8, y, x + w - 8, y + 1, Theme.BORDER);
			String when = r.label().isBlank() ? r.when() : r.when() + "  " + r.label();
			ui.text(ui.fit(when, c2 - c1 - 20), c1, y + 9, Theme.TEXT);
			ui.textRight(String.format("%.0f", r.avgFps()), c2 + 40, y + 9, Theme.TEXT);
			ui.textRight(String.format("%.0f", r.low1()), c3 + 40, y + 9, Theme.TEXT);
			ui.textRight(String.format("%.0f", r.low01()), c4 + 40, y + 9, Theme.TEXT);
			y += 18;
		}
		return y;
	}

	private int bigNumbers(int x, int y, double avg, double low1, double low01) {
		int col = 0;
		for (String[] kv : new String[][] {
			{String.format("%.0f", avg), "fps average"},
			{String.format("%.0f", low1), "1% low"},
			{String.format("%.0f", low01), "0.1% low"},
		}) {
			int cx = x + col * 120;
			ui.bold(kv[0], cx, y + 6, Theme.TEXT);
			ui.text(kv[1], cx, y + 18, Theme.TEXT_DIM);
			col++;
		}
		return y + 26;
	}

	private ExileMenuScreen withResult(BenchResult result) {
		ExileMenuScreen s = new ExileMenuScreen(new TitleScreen(), Tab.BENCH);
		s.lastResult = result;
		return s;
	}

	@Override
	public boolean isPauseScreen() {
		return true;
	}
}
