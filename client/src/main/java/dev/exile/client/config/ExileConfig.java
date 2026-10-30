package dev.exile.client.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import dev.exile.client.ExileClient;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.LinkedHashMap;
import java.util.Map;
import net.fabricmc.loader.api.FabricLoader;

/**
 * Exile's settings, stored as {@code config/exile.json}. Every field has a
 * default, so missing or older files load cleanly; unknown fields are ignored.
 */
public final class ExileConfig {
	private static final Path FILE = FabricLoader.getInstance().getConfigDir().resolve("exile.json");
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
	private static ExileConfig instance;
	private static final int LAYOUT_VERSION = 2;

	/**
	 * Bumped when default HUD positions change; see {@link #normalised}. No
	 * initialiser: files from before it existed must read as 0.
	 */
	public int layoutVersion;
	/** HUD element settings by element id. */
	public Map<String, HudSlot> hud = new LinkedHashMap<>();
	/** Draw a subtle backdrop behind HUD elements. */
	public boolean hudBackground = true;

	public Features features = new Features();
	public Performance performance = new Performance();

	public static final class HudSlot {
		public boolean enabled;
		/** Position as a fraction (0..1) of the free space, so layouts survive resizing. */
		public float x;
		public float y;
		/** Size relative to normal (0.5..2). */
		public float scale = 1f;
		/** What to show, for elements that offer a choice: "full", "icon" or "text". */
		public String mode = "full";

		public HudSlot() {}

		public HudSlot(boolean enabled, float x, float y) {
			this.enabled = enabled;
			this.x = x;
			this.y = y;
		}
	}

	public static final class Features {
		public boolean freelook = true;
		/** Hide vanilla's effect icons when Exile's effects list is shown. */
		public boolean replaceVanillaEffects = true;
		/** Gold outline on the hunger bar for the saturation left. */
		public boolean saturationOverlay = true;

		public boolean zoom = true;
		/** Press once to zoom and again to stop, instead of holding the key. */
		public boolean zoomToggle = false;
		/** Magnification when zooming starts (2x..10x). */
		public int zoomLevel = 4;
		/** Ease in and out instead of jumping. */
		public boolean zoomSmooth = true;
		/** Mouse wheel changes the zoom while zoomed. */
		public boolean zoomScroll = true;
		/** Vanilla's cinematic camera while zoomed. */
		public boolean zoomCinematic = false;

		/** Armour drawn closer to the body (all humanoid players). */
		public boolean slimArmour = false;
		/** Don't draw your own armour (only on this client). */
		public boolean invisibleArmour = false;

		/** See as far underwater as in air. */
		public boolean clearWater = false;
		/** See through lava. */
		public boolean clearLava = false;
		/** Fire on the screen sits lower, out of the middle. */
		public boolean lowFire = false;
		/** The first-person shield sits lower. */
		public boolean lowShield = false;
		/** Everything lit as if in full light. */
		public boolean fullbright = false;
		/** Vanilla menus: "vanilla", "dark" or "exile". */
		public String menuTheme = "vanilla";
		/** Exile's own screens and HUD: "launcher" (follow the launcher), "exile" or "dark". */
		public String uiTheme = "launcher";

		/** Your own name tag above your head in third person. */
		public boolean ownNameTag = false;
		/** Exile badges next to player names (looked up on exiles.dev). */
		public boolean badges = true;
	}

	public static final class Performance {
		/** Frame cap while the window is visible but not focused (on/off is a Tuning switch). */
		public int backgroundFps = 30;
		/** Frame Budget governor: hold a target frame rate. */
		public boolean governor = false;
		public int governorTargetFps = 60;
		/** Settings the governor started from, restored when it stops. */
		public Map<String, Double> governorBaseline = new LinkedHashMap<>();
		public boolean showFrameBudget = false;
		/** Individual optimisations, by Tuning switch name. */
		public Map<String, Boolean> optimisations = new LinkedHashMap<>();
	}

	private void moveIfAt(String id, float oldX, float oldY) {
		HudSlot s = hud.get(id);
		if (s != null && s.x == oldX && s.y == oldY) {
			s.x = 0f;
			s.y = 0f;
		}
	}

	public static ExileConfig get() {
		if (instance == null) instance = load();
		return instance;
	}

	private static ExileConfig load() {
		if (Files.exists(FILE)) {
			try {
				ExileConfig c = GSON.fromJson(Files.readString(FILE, StandardCharsets.UTF_8), ExileConfig.class);
				if (c != null) return c.normalised();
			} catch (IOException | RuntimeException e) {
				ExileClient.LOG.warn("Couldn't read {}, using defaults", FILE, e);
			}
		}
		return new ExileConfig().normalised();
	}

	/** Fills in anything a partial file left null. */
	private ExileConfig normalised() {
		if (hud == null) hud = new LinkedHashMap<>();
		if (features == null) features = new Features();
		if (performance == null) performance = new Performance();
		if (performance.optimisations == null) performance.optimisations = new LinkedHashMap<>();
		if (performance.governorBaseline == null) performance.governorBaseline = new LinkedHashMap<>();
		performance.backgroundFps = Math.clamp(performance.backgroundFps, 5, 120);
		performance.governorTargetFps = Math.clamp(performance.governorTargetFps, 30, 360);
		features.zoomLevel = Math.clamp(features.zoomLevel, 2, 10);
		if (features.menuTheme == null) features.menuTheme = "vanilla";
		if (features.uiTheme == null) features.uiTheme = "launcher";
		for (HudSlot slot : hud.values()) {
			if (slot.mode == null) slot.mode = "full";
			slot.scale = Float.isFinite(slot.scale) && slot.scale > 0 ? Math.clamp(slot.scale, 0.5f, 2f) : 1f;
		}
		if (layoutVersion < 2) {
			// Version 2 stacks the left column from the top corner (the layout
			// keeps elements from overlapping, so they line up evenly at every
			// GUI scale). Move elements still at their old defaults; leave any
			// the player placed.
			moveIfAt("coordinates", 0f, 0.09f);
			moveIfAt("cps", 0f, 0.19f);
			moveIfAt("frame_budget", 0f, 0.3f);
			layoutVersion = LAYOUT_VERSION;
		}
		return this;
	}

	public void save() {
		try {
			Files.createDirectories(FILE.getParent());
			Path tmp = FILE.resolveSibling("exile.json.tmp");
			Files.writeString(tmp, GSON.toJson(this), StandardCharsets.UTF_8);
			Files.move(tmp, FILE, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
		} catch (IOException e) {
			ExileClient.LOG.warn("Couldn't save {}", FILE, e);
		}
	}

	public HudSlot slot(String id, boolean defaultEnabled, float defaultX, float defaultY) {
		return hud.computeIfAbsent(id, k -> new HudSlot(defaultEnabled, defaultX, defaultY));
	}

	public boolean optimisation(String name, boolean defaultOn) {
		return performance.optimisations.getOrDefault(name, defaultOn);
	}
}
