package dev.exile.client.ui;

import dev.exile.client.config.ExileConfig;
import dev.exile.client.feature.Keybinds;
import dev.exile.client.hud.ExileHud;
import dev.exile.client.hud.HudElement;
import dev.exile.client.perf.BackgroundThrottle;
import dev.exile.client.perf.Tuning;
import dev.exile.client.ui.ExileMenuScreen.Tab;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;

/**
 * Every card in the Exile menu, in display order: grouped by tab, then by
 * section. Settings beyond a card's switch live on its detail page, so the
 * grids stay one glance wide.
 */
final class MenuCards {
	private MenuCards() {}

	private static final float[] SIZES = {0.5f, 0.75f, 1f, 1.25f, 1.5f, 1.75f, 2f};

	static List<Card> all() {
		List<Card> cards = new ArrayList<>();
		hud(cards);
		features(cards);
		performance(cards);
		return cards;
	}

	// -------------------------------------------------------------------- HUD

	private static void hud(List<Card> cards) {
		var cfg = ExileConfig.get();
		cards.add(Card.of(Tab.HUD, "Layout", "Edit layout", "Drag elements to where you want them, and resize them")
			.summary(() -> Minecraft.getInstance().player != null ? "Drag elements into place" : "Join a world to edit")
			.action(s -> s.openHudEditor(), () -> Minecraft.getInstance().player != null)
			.keywords("move position arrange drag resize"));
		cards.add(Card.of(Tab.HUD, "Layout", "Backdrop", "A subtle dark box behind each element")
			.toggle(() -> cfg.hudBackground, on -> cfg.hudBackground = on)
			.keywords("background box"));

		for (HudElement e : ExileHud.ELEMENTS) {
			Card c = Card.of(Tab.HUD, "Elements", e.name, e.description)
				.toggle(() -> e.slot().enabled, on -> e.slot().enabled = on)
				.detail((s, x, y, w) -> element(s, e, x, y, w))
				.keywords("hud overlay element");
			if (e == ExileHud.EFFECTS) c.keywords("hud overlay element potion status icons");
			if (e == ExileHud.FRAME_BUDGET) c.keywords("hud overlay element performance frame time ms");
			cards.add(c);
		}
	}

	private static int element(ExileMenuScreen s, HudElement e, int x, int y, int w) {
		if (!e.modes().isEmpty()) {
			String[] labels = e.modes().stream().map(m -> m.label).toArray(String[]::new);
			int active = e.modes().indexOf(e.mode());
			y = s.choiceRow(x, y, w, "Show", "Icons and text, icons only, or text only", labels, active, i -> e.setMode(e.modes().get(i)));
		}
		int pct = Math.round(e.scale() * 100);
		// The slider tracks its drag by identity: the element is a stable id.
		y = s.sliderRow(x, y, w, "Size", e, pct, 50, 200, 25, "%", v -> e.slot().scale = nearestSize(v / 100f));
		if (e == ExileHud.EFFECTS) {
			var f = ExileConfig.get().features;
			y = s.toggleRow(
				x, y, w, "Replace effect icons", "Hide vanilla's effect icons while this list is on", f.replaceVanillaEffects,
				on -> f.replaceVanillaEffects = on
			);
		}
		return s.note(x, y, w, "Move it under Edit layout, on the HUD tab.");
	}

	private static float nearestSize(float scale) {
		float best = SIZES[0];
		for (float size : SIZES) if (Math.abs(size - scale) < Math.abs(best - scale)) best = size;
		return best;
	}

	// --------------------------------------------------------------- features

	private static void features(List<Card> cards) {
		var f = ExileConfig.get().features;
		var o = Minecraft.getInstance().options;

		cards.add(Card.of(Tab.FEATURES, "Camera", "Free look", "Hold the key to look around without turning")
			.toggle(() -> f.freelook, on -> f.freelook = on)
			.summary(() -> "Hold " + keyName(Keybinds.freelook))
			.detail((s, x, y, w) -> s.keyRow(x, y, w, "Key", Keybinds.freelook))
			.keywords("perspective look around alt"));
		cards.add(Card.of(Tab.FEATURES, "Camera", "Zoom", "Hold the key to magnify the view")
			.toggle(() -> f.zoom, on -> f.zoom = on)
			.summary(() -> f.zoomLevel + "x, " + (f.zoomToggle ? "press " : "hold ") + keyName(Keybinds.zoom))
			.detail((s, x, y, w) -> {
				y = s.keyRow(x, y, w, "Key", Keybinds.zoom);
				y = s.sliderRow(x, y, w, "Amount", "zoom", f.zoomLevel, 2, 10, 1, "x", v -> f.zoomLevel = v);
				y = s.toggleRow(x, y, w, "Toggle zoom", "Press once to zoom and again to stop, instead of holding", f.zoomToggle, on -> f.zoomToggle = on);
				y = s.toggleRow(x, y, w, "Smooth zoom", "Ease in and out instead of jumping", f.zoomSmooth, on -> f.zoomSmooth = on);
				y = s.toggleRow(x, y, w, "Scroll to adjust", "Mouse wheel zooms further in or out while zoomed", f.zoomScroll, on -> f.zoomScroll = on);
				return s.toggleRow(x, y, w, "Cinematic camera", "Smooth, drifting camera movement while zoomed", f.zoomCinematic, on -> f.zoomCinematic = on);
			})
			.keywords("magnify spyglass optifine scroll cinematic"));

		cards.add(Card.of(Tab.FEATURES, "Visibility", "Fullbright", "See everything as if in full light")
			.toggle(() -> f.fullbright, on -> f.fullbright = on)
			.summary(() -> Keybinds.fullbright.isUnbound() ? "No toggle key" : "Toggle with " + keyName(Keybinds.fullbright))
			.detail((s, x, y, w) -> s.keyRow(x, y, w, "Toggle key", Keybinds.fullbright))
			.keywords("gamma brightness night vision light"));
		simple(cards, Tab.FEATURES, "Visibility", "Clear water", "See as far underwater as on land", () -> f.clearWater, on -> f.clearWater = on, "fog underwater");
		simple(cards, Tab.FEATURES, "Visibility", "Clear lava", "See through lava when you're in it", () -> f.clearLava, on -> f.clearLava = on, "fog");
		simple(cards, Tab.FEATURES, "Visibility", "Low fire", "Flames on screen sit lower when you're burning", () -> f.lowFire, on -> f.lowFire = on, "burning overlay");
		simple(cards, Tab.FEATURES, "Visibility", "Low shield", "Your shield sits lower, so it blocks less of the view", () -> f.lowShield, on -> f.lowShield = on, "offhand");

		simple(cards, Tab.FEATURES, "Movement", "Toggle sprint", "Press sprint once to keep sprinting", () -> o.toggleSprint().get(), on -> o.toggleSprint().set(on), "run");
		simple(cards, Tab.FEATURES, "Movement", "Toggle sneak", "Press sneak once to keep sneaking", () -> o.toggleCrouch().get(), on -> o.toggleCrouch().set(on), "crouch shift");

		simple(cards, Tab.FEATURES, "Players", "Your name tag", "Show your name above your head in third person", () -> f.ownNameTag, on -> f.ownNameTag = on, "nametag own self");
		simple(cards, Tab.FEATURES, "Players", "Badges", "Exile badges next to player names, from exiles.dev", () -> f.badges, on -> f.badges = on, "icons tab list");
		simple(cards, Tab.FEATURES, "Players", "Slim armour", "Armour sits closer to the body, on every player", () -> f.slimArmour, on -> f.slimArmour = on, "armor");
		simple(
			cards, Tab.FEATURES, "Players", "Invisible armour", "Hide your own armour so your skin shows. Only you see this", () -> f.invisibleArmour,
			on -> f.invisibleArmour = on, "armor hide skin"
		);

		cards.add(Card.of(Tab.FEATURES, "Interface", "Exile theme", "The palette of Exile's menus and HUD, like the launcher's")
			.summary(MenuCards::themeSummary)
			.detail((s, x, y, w) -> {
				Theme.Mode[] modes = Theme.Mode.values();
				String[] labels = new String[modes.length];
				for (int i = 0; i < modes.length; i++) labels[i] = modes[i].label;
				y = s.choiceRow(x, y, w, "Theme", "Blue-tinted or neutral greys", labels, Theme.mode().ordinal(), i -> Theme.setMode(modes[i]));
				String note = Theme.launcherKnown()
					? "Match launcher follows the theme set in the launcher's Appearance settings when the game starts."
					: "This game wasn't started by the Exile launcher, so Match launcher uses Exile.";
				return s.note(x, y, w, note);
			})
			.keywords("dark mode colour color palette appearance launcher"));
		cards.add(Card.of(Tab.FEATURES, "Interface", "Menu theme", "Tints Minecraft's own menus and inventories")
			.summary(() -> switch (MenuTheme.mode()) {
				case VANILLA -> "Vanilla";
				case DARK -> "Dark";
				case EXILE -> "Exile";
			})
			.detail((s, x, y, w) -> {
				String[] labels = {"Vanilla", "Dark", "Exile"};
				y = s.choiceRow(x, y, w, "Theme", "Darker vanilla menus", labels, MenuTheme.mode().ordinal(), i -> MenuTheme.set(MenuTheme.Mode.values()[i]));
				return s.note(x, y, w, "Tints rather than replaces textures, so it works with resource packs.");
			})
			.keywords("dark mode inventory container gui appearance"));
		simple(
			cards, Tab.FEATURES, "Interface", "Hunger bar saturation", "Gold outlines on the hunger bar show the saturation left",
			() -> f.saturationOverlay, on -> f.saturationOverlay = on, "food"
		);
		cards.add(Card.of(Tab.FEATURES, "Interface", "Menu key", "Opens this menu from anywhere in game")
			.summary(() -> keyName(Keybinds.menu))
			.detail((s, x, y, w) -> {
				y = s.keyRow(x, y, w, "Key", Keybinds.menu);
				return s.note(x, y, w, "Exile's keys are also under Options, Controls, Exile.");
			})
			.keywords("keybind hotkey right shift open"));
	}

	// ------------------------------------------------------------ performance

	private static void performance(List<Card> cards) {
		var p = ExileConfig.get().performance;
		var bt = BackgroundThrottle.SWITCH;
		cards.add(Card.of(Tab.PERFORMANCE, "Frame rate", bt.title, "Limit FPS in another window (about 70% less GPU power in testing)")
			.toggle(bt::enabled, bt::set)
			.locked(bt::forced)
			.summary(() -> bt.forced() ? "Set by launch options" : p.backgroundFps + " fps when unfocused")
			.detail((s, x, y, w) -> s.sliderRow(x, y, w, "Background FPS", "bg", p.backgroundFps, 5, 120, 5, " fps", v -> p.backgroundFps = v))
			.keywords("unfocused alt tab power battery gpu"));
		cards.add(Card.of(Tab.PERFORMANCE, "Frame rate", "Target frame rate", "Holds a frame rate by trimming the least noticeable settings first")
			.toggle(() -> p.governor, on -> p.governor = on)
			.summary(() -> "Hold " + p.governorTargetFps + " fps")
			.detail((s, x, y, w) -> s.sliderRow(x, y, w, "Target", "target", p.governorTargetFps, 30, 240, 10, " fps", v -> p.governorTargetFps = v))
			.keywords("governor fps frame budget dynamic"));

		for (Tuning.Switch sw : Tuning.all()) {
			if (sw == bt) continue;
			cards.add(Card.of(Tab.PERFORMANCE, "Optimisations", sw.title, sw.description)
				.toggle(sw::enabled, sw::set)
				.locked(sw::forced)
				.summary(() -> sw.forced() ? "Set by launch options" : sw.description)
				.keywords("optimization optimisation " + sw.name.replace('_', ' ')));
		}
	}

	// ---------------------------------------------------------------- helpers

	/** A card that is just a switch. */
	private static void simple(
		List<Card> cards, Tab tab, String section, String title, String description, java.util.function.BooleanSupplier on,
		java.util.function.Consumer<Boolean> set, String keywords
	) {
		cards.add(Card.of(tab, section, title, description).toggle(on, set).keywords(keywords));
	}

	static String keyName(KeyMapping key) {
		return key.isUnbound() ? "no key" : key.getTranslatedKeyMessage().getString();
	}

	private static String themeSummary() {
		Theme.Mode mode = Theme.mode();
		if (mode != Theme.Mode.LAUNCHER) return mode.label;
		return mode.label + " (" + (Theme.dark() ? "Dark" : "Exile") + ")";
	}
}
