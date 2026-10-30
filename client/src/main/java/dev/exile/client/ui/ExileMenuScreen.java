package dev.exile.client.ui;

import com.mojang.blaze3d.Blaze3D;
import com.mojang.blaze3d.platform.InputConstants;
import dev.exile.client.bench.BenchHistory;
import dev.exile.client.bench.BenchResult;
import dev.exile.client.bench.ExileBench;
import dev.exile.client.config.ExileConfig;
import dev.exile.client.ui.Ui.Kind;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.IntConsumer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

/**
 * The Exile menu: HUD, features, performance, mods and benchmark in one
 * minimal panel. Opened from the pause menu or with Right Shift.
 *
 * <p>Settings are cards in a grid (see {@link MenuCards}): a name, a one-line
 * summary and a switch. Cards with more to set open a detail page. Typing
 * anywhere searches every card.
 */
public final class ExileMenuScreen extends Screen {
	public enum Tab {
		HUD("HUD", "What's on screen, and where."),
		FEATURES("Features", "Camera, visibility, players and interface. All client-side."),
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

	private static final int SIDE = 120;
	private static final int CARD_H = 36;
	private static final int GAP = 6;
	private static final int MIN_CARD_W = 132;

	private final Screen parent;
	private final Ui ui = new Ui();
	private final List<Card> cards = MenuCards.all();
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
	/** Search text; when not empty, the content shows matching cards from every tab. */
	private String query = "";
	private boolean searchFocused;
	/** The card whose detail page is showing, if any. */
	private Card open;

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
		query = "";
		open = null;
		capturing = null;
		if (t == Tab.MODS && mods == null) mods = ModEntries.load();
		if (t == Tab.BENCH) history = BenchHistory.recent(6);
	}

	private boolean searching() {
		return !query.isBlank();
	}

	private void setQuery(String q) {
		query = q;
		open = null;
		capturing = null;
		scroll = 0;
	}

	private void openCard(Card c) {
		open = c;
		capturing = null;
		scroll = 0;
	}

	private void back() {
		open = null;
		capturing = null;
		scroll = 0;
	}

	void openHudEditor() {
		minecraft.gui.setScreen(new HudEditorScreen(this));
	}

	// ---------------------------------------------------------------- input

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		// While rebinding, any button but the left one becomes the key.
		if (capturing != null && event.button() != InputConstants.MOUSE_BUTTON_LEFT) {
			bind(InputConstants.Type.MOUSE.getOrCreate(event.button()));
			return true;
		}
		if (event.button() == InputConstants.MOUSE_BUTTON_LEFT) {
			// The search box claims the click back if it was hit.
			searchFocused = false;
			ui.press(event.x(), event.y());
		}
		// Back and forward mouse buttons step out of a detail page.
		if (event.button() == 3 && open != null) back();
		return true;
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		if (capturing != null) {
			if (event.key() == InputConstants.KEY_ESCAPE) capturing = null;
			else bind(InputConstants.getKey(event));
			return true;
		}
		// Esc steps back one level: a detail page, then the search, then the menu.
		if (event.key() == InputConstants.KEY_ESCAPE) {
			if (open != null) {
				back();
				return true;
			}
			if (searching() || searchFocused) {
				setQuery("");
				searchFocused = false;
				return true;
			}
			return super.keyPressed(event);
		}
		if (event.key() == InputConstants.KEY_F && event.hasControlDown()) {
			searchFocused = true;
			return true;
		}
		if (event.isPaste()) {
			String clip = minecraft.keyboardHandler.getClipboard().replaceAll("\\s+", " ").strip();
			if (!clip.isEmpty()) {
				setQuery((query + clip).substring(0, Math.min(64, query.length() + clip.length())));
				searchFocused = true;
			}
			return true;
		}
		if (event.key() == InputConstants.KEY_BACKSPACE && !query.isEmpty()) {
			setQuery(event.hasControlDown() ? "" : query.substring(0, query.offsetByCodePoints(query.length(), -1)));
			searchFocused = true;
			return true;
		}
		return super.keyPressed(event);
	}

	/** Typing anywhere searches. */
	@Override
	public boolean charTyped(CharacterEvent event) {
		if (capturing != null || !event.isAllowedChatCharacter()) return false;
		if (query.isEmpty() && event.codepoint() == ' ') return true;
		if (query.length() < 64) setQuery(query + event.codepointAsString());
		searchFocused = true;
		return true;
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

	/**
	 * Typing anywhere searches, so the screen takes text input for as long as
	 * it is open. Without it the game sends no characters at all (only key
	 * presses): they are reserved for whatever asked for text input.
	 */
	@Override
	protected void init() {
		minecraft.textInputManager().startTextInput(this);
	}

	/** Called whenever this screen goes away, including when the game quits with it open. */
	@Override
	public void removed() {
		minecraft.textInputManager().stopTextInput(this);
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
		int pw = Math.min(width - 24, 620);
		int ph = Math.min(height - 24, 380);
		int px = (width - pw) / 2;
		int py = (height - ph) / 2;
		ui.panel(px, py, pw, ph, Theme.BG, Theme.BORDER_STRONG);

		sidebar(g, px, py, ph);

		// Content
		int cx = px + SIDE + 20;
		int cw = pw - SIDE - 40;
		int top = py + 18;
		header(cx, top, cw);

		int bodyTop = top + 34;
		int bodyBottom = py + ph - 12;
		int maxScroll = Math.max(0, contentHeight - (bodyBottom - bodyTop));
		scroll = Math.clamp(scroll, 0, maxScroll);
		ui.clip(cx - 4, bodyTop, cx + cw + 4, bodyBottom);
		int y = bodyTop - (int) scroll;
		int end;
		if (open != null) end = detail(open, cx, y, cw);
		else if (searching()) end = searchResults(cx, y, cw);
		else end = switch (tab) {
			case HUD, FEATURES -> grid(cardsIn(tab), false, cx, y, cw);
			case PERFORMANCE -> grid(cardsIn(tab), false, cx, vsyncNote(cx, y, cw), cw);
			case MODS -> modsTab(cx, y, cw, bodyTop, bodyBottom);
			case BENCH -> benchTab(cx, y, cw);
		};
		ui.endClip();
		contentHeight = end - y;
		if (maxScroll > 0) scrollbar(cx + cw + 6, bodyTop, bodyBottom - bodyTop, maxScroll);
		ui.end();
	}

	private void sidebar(GuiGraphicsExtractor g, int px, int py, int ph) {
		g.fill(px + 1, py + 1, px + SIDE, py + ph - 1, Theme.SIDEBAR);
		g.fill(px + SIDE, py + 1, px + SIDE + 1, py + ph - 1, Theme.BORDER);
		brand(px + 14, py + 20);

		int sx = px + 8, sw = SIDE - 16, sy = py + 34, sh = 18;
		if (ui.searchField(sx, sy, sw, sh, query, "Search", searchFocused)) searchFocused = true;
		if (!query.isEmpty() && ui.clearButton(sx + sw - 2, sy, sh)) {
			setQuery("");
			searchFocused = false;
		}

		int ty = sy + sh + 10;
		for (Tab t : Tab.values()) {
			int tx = px + 8, tw = SIDE - 16, th = 20;
			// While searching no tab is current: results come from all of them.
			boolean selected = t == tab && !searching();
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
	}

	/** Title and subtitle; on a detail page the title has a back arrow. */
	private void header(int cx, int top, int cw) {
		if (open != null) {
			int tw = 10 + ui.width(Theme.bold(open.title));
			boolean hovered = ui.interactive(cx - 2, top - 4, tw + 4, 16);
			int color = hovered ? Theme.TEXT : Theme.TEXT_DIM;
			ui.chevronBack(cx, top + 4, color);
			ui.bold(open.title, cx + 10, top + 4, Theme.TEXT);
			if (ui.click(cx - 2, top - 4, tw + 4, 16)) back();
			String from = searching() ? "Search" : open.tab.title + ", " + open.section;
			ui.text(ui.fit(from + " · " + open.description, cw), cx, top + 18, Theme.TEXT_DIM);
			return;
		}
		if (searching()) {
			long n = cards.stream().filter(c -> c.matches(query)).count();
			ui.bold("Search", cx, top + 4, Theme.TEXT);
			String sub = n == 0 ? "Nothing matches “" + query.strip() + "”" : n + (n == 1 ? " setting" : " settings");
			ui.text(ui.fit(sub, cw), cx, top + 18, Theme.TEXT_DIM);
			return;
		}
		ui.bold(tab.title, cx, top + 4, Theme.TEXT);
		String sub = tab == Tab.MODS ? (mods == null ? "" : mods.size() + " mods loaded") : tab.subtitle;
		ui.text(ui.fit(sub, cw), cx, top + 18, Theme.TEXT_DIM);
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

	// ---------------------------------------------------------------- cards

	private List<Card> cardsIn(Tab t) {
		return cards.stream().filter(c -> c.tab == t).toList();
	}

	private int searchResults(int x, int y, int w) {
		List<Card> found = cards.stream().filter(c -> c.matches(query)).toList();
		if (found.isEmpty()) {
			return y + ui.paragraph("Try another word, or press Esc to go back.", x + 2, y + 4, w, Theme.TEXT_FAINT) + 8;
		}
		return grid(found, true, x, y, w);
	}

	/**
	 * Cards in a grid, under a heading per section (per tab for search
	 * results). As many columns as fit; every card the same size.
	 */
	private int grid(List<Card> list, boolean byTab, int x, int y, int w) {
		int cols = Math.max(1, (w + GAP) / (MIN_CARD_W + GAP));
		int cardW = (w - GAP * (cols - 1)) / cols;
		String heading = null;
		int col = 0;
		for (Card c : list) {
			String h = byTab ? c.tab.title : c.section;
			if (!h.equals(heading)) {
				if (heading != null) y += (col > 0 ? CARD_H : -GAP) + 12;
				heading = h;
				col = 0;
				ui.section(h, x + 2, y + 4);
				y += 12;
			}
			card(c, x + col * (cardW + GAP), y, cardW);
			if (++col == cols) {
				col = 0;
				y += CARD_H + GAP;
			}
		}
		return y + (col > 0 ? CARD_H : 0) + 6;
	}

	private void card(Card c, int x, int y, int w) {
		boolean locked = c.locked.getAsBoolean();
		boolean on = c.hasToggle() && c.on.getAsBoolean();
		boolean opens = c.detail != null || c.action != null;
		boolean clickable = c.action != null ? c.actionEnabled.getAsBoolean() : opens || (c.hasToggle() && !locked);
		boolean hovered = clickable ? ui.interactive(x, y, w, CARD_H) : ui.hover(x, y, w, CARD_H);
		ui.panel(x, y, w, CARD_H, hovered ? Theme.SURFACE_HOVER : Theme.SURFACE, hovered ? Theme.BORDER_STRONG : Theme.BORDER);

		int right = x + w - 8;
		int titleRoom = w - 16;
		if (c.hasToggle()) {
			boolean next = ui.toggle(right - 22, y + 6, on, !locked);
			if (next != on) c.set.accept(next);
			titleRoom -= 28;
		} else if (opens) {
			ui.chevron(right - 4, y + 12, hovered ? Theme.TEXT : Theme.TEXT_FAINT);
			titleRoom -= 10;
		}
		// Off switches read dimmer, so what's on stands out across the grid.
		boolean bright = !c.hasToggle() || on;
		int titleColor = c.action != null && !clickable ? Theme.TEXT_FAINT : bright ? Theme.TEXT : Theme.TEXT_DIM;
		ui.text(ui.fit(c.title, titleRoom), x + 8, y + 12, titleColor);

		String summary = c.summaryText();
		int summaryRoom = w - 16;
		if (c.hasToggle() && c.detail != null) {
			ui.chevron(right - 4, y + 25, hovered ? Theme.TEXT_DIM : Theme.TEXT_FAINT);
			summaryRoom -= 10;
		}
		String shown = ui.fit(summary, summaryRoom);
		ui.text(shown, x + 8, y + 25, Theme.TEXT_FAINT);

		// The full description, when the card doesn't already show all of it.
		if (hovered && (!shown.equals(c.description) || locked)) {
			ui.tooltip(c, locked ? c.description + " (set by launch options)" : c.description);
		}
		// The switch above claimed its own clicks; the rest of the card is one button.
		if (clickable && ui.click(x, y, w, CARD_H)) {
			if (c.action != null) c.action.accept(this);
			else if (c.detail != null) openCard(c);
			else c.set.accept(!on);
		}
	}

	/** A card's own page: its switch, then its settings. */
	private int detail(Card c, int x, int y, int w) {
		if (c.hasToggle()) {
			boolean locked = c.locked.getAsBoolean();
			boolean on = c.on.getAsBoolean();
			boolean next = ui.toggleRow(x, y, w, "Enabled", locked ? "Set by launch options" : null, on, !locked);
			if (next != on) c.set.accept(next);
			y += locked ? 30 : 22;
			ui.hline(x + 8, x + w - 8, y + 2, Theme.BORDER);
			y += 8;
		}
		return c.detail == null ? y : c.detail.draw(this, x, y, w);
	}

	private int vsyncNote(int x, int y, int w) {
		// VSync caps everything below at the display's refresh rate, which is
		// easy to mistake for one of these settings holding the frame rate.
		if (!minecraft.options.enableVsync().get()) return y;
		var monitor = minecraft.getWindow().findBestMonitor();
		int hz = monitor == null ? 0 : Math.round(monitor.currentMode().getRefreshRate());
		String cap = hz > 0 ? hz + " Hz" : "refresh rate";
		return y + ui.paragraph("VSync is on, so FPS stops at your display's " + cap + ". Turn it off in Video Settings to go higher.", x + 2, y + 2, w - 4, Theme.TEXT_DIM) + 10;
	}

	// ------------------------------------------------------- detail rows (used by MenuCards)

	/** A labelled switch. Returns where the next row starts. */
	int toggleRow(int x, int y, int w, String label, String detail, boolean on, Consumer<Boolean> set) {
		boolean next = ui.toggleRow(x, y, w, label, detail, on, true);
		if (next != on) set.accept(next);
		return y + (detail == null ? 22 : 30);
	}

	/** One of a few choices, on the right of a label. */
	int choiceRow(int x, int y, int w, String label, String detail, String[] options, int active, IntConsumer set) {
		int h = detail == null ? 22 : 30;
		int cy = detail == null ? y + h / 2 : y + 9;
		int segW = ui.segmentedWidth(options);
		ui.text(label, x + 8, cy, Theme.TEXT);
		if (detail != null) ui.text(ui.fit(detail, w - segW - 24), x + 8, y + 20, Theme.TEXT_FAINT);
		int picked = ui.segmented(x + w - 8 - segW, y + (h - 18) / 2, 18, options, active);
		if (picked != active) set.accept(picked);
		return y + h + 2;
	}

	/**
	 * A key binding with its key on a button. Click it, then press a key (or a
	 * side or middle mouse button) to rebind; Esc or clicking again cancels.
	 */
	int keyRow(int x, int y, int w, String label, KeyMapping key) {
		boolean listening = capturing == key;
		ui.text(label, x + 8, y + 10, Theme.TEXT);
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

	int sliderRow(int x, int y, int w, String label, Object id, int value, int min, int max, int step, String unit, IntConsumer set) {
		ui.text(label, x + 8, y + 10, Theme.TEXT);
		ui.textRight(value + unit, x + w - 8, y + 10, Theme.TEXT);
		int sx = x + 110;
		int next = ui.slider(id, sx, y + 4, w - 110 - 60, value, min, max, step);
		if (next != value) set.accept(next);
		return y + 24;
	}

	/** Faint explanatory text. */
	int note(int x, int y, int w, String text) {
		return y + 4 + ui.paragraph(text, x + 8, y + 4, w - 16, Theme.TEXT_FAINT) + 6;
	}

	// ---------------------------------------------------------------- tabs

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
