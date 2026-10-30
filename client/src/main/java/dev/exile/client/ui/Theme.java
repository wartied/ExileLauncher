package dev.exile.client.ui;

import dev.exile.client.config.ExileConfig;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.resources.Identifier;

/**
 * Exile's look, shared with the launcher: flat surfaces, Iosevka Aile text,
 * and the launcher's two palettes. Exile's greys are tinted with the brand
 * blue #678FC5; Dark uses the same lightness steps in neutral grey. The
 * player picks one in the Exile menu, or follows whatever the launcher that
 * started the game is using.
 */
public final class Theme {
	private Theme() {}

	public enum Mode {
		LAUNCHER("Match launcher"),
		EXILE("Exile"),
		DARK("Dark");

		public final String label;

		Mode(String label) {
			this.label = label;
		}
	}

	/** Set by the Exile launcher when it starts the game ("exile" or "dark"). */
	private static final String LAUNCHER_THEME = System.getenv("EXILE_LAUNCHER_THEME");

	// The neutrals (ARGB), set by refresh(). Accent and status colours are shared.
	public static int BACKDROP;
	public static int BG;
	public static int SIDEBAR;
	public static int SURFACE;
	public static int SURFACE_HOVER;
	public static int SURFACE_ACTIVE;
	public static int BORDER;
	public static int BORDER_STRONG;
	public static int TEXT;
	public static int TEXT_DIM;
	public static int TEXT_FAINT;
	/** HUD element backdrop: dark and translucent so the world stays visible. */
	public static int HUD_BACKDROP;

	public static final int ACCENT = 0xFF678FC5;
	public static final int ACCENT_LIGHT = 0xFF8EAEDB;
	public static final int ACCENT_DIM = 0xFF2B3E5A;
	public static final int BUTTON = 0xFF4F79B2;
	public static final int BUTTON_HOVER = 0xFF5A84BD;
	public static final int BUTTON_PRESSED = 0xFF466CA0;
	public static final int DANGER = 0xFFE5534B;
	public static final int WARN = 0xFFD9A23B;

	static {
		refresh();
	}

	/** The chosen mode (config/exile.json). */
	public static Mode mode() {
		return switch (ExileConfig.get().features.uiTheme) {
			case "exile" -> Mode.EXILE;
			case "dark" -> Mode.DARK;
			default -> Mode.LAUNCHER;
		};
	}

	public static void setMode(Mode mode) {
		ExileConfig.get().features.uiTheme = mode.name().toLowerCase();
		refresh();
	}

	/** Whether the neutral Dark palette is showing; Match launcher resolves to the launcher's choice. */
	public static boolean dark() {
		return switch (mode()) {
			case DARK -> true;
			case EXILE -> false;
			case LAUNCHER -> "dark".equalsIgnoreCase(LAUNCHER_THEME);
		};
	}

	/** True when the game was started by the Exile launcher, so Match launcher means something. */
	public static boolean launcherKnown() {
		return LAUNCHER_THEME != null && !LAUNCHER_THEME.isBlank();
	}

	/** Loads the active palette into the colour fields. */
	public static void refresh() {
		if (dark()) {
			BG = 0xFF131313;
			SIDEBAR = 0xFF171717;
			SURFACE = 0xFF1E1E1E;
			SURFACE_HOVER = 0xFF252525;
			SURFACE_ACTIVE = 0xFF2D2D2D;
			BORDER = 0xFF292929;
			BORDER_STRONG = 0xFF383838;
			TEXT = 0xFFE8E8E8;
			TEXT_DIM = 0xFFA0A0A0;
			TEXT_FAINT = 0xFF6C6C6C;
			BACKDROP = 0xB00A0A0A;
		} else {
			BG = 0xFF111419;
			SIDEBAR = 0xFF151920;
			SURFACE = 0xFF1B2029;
			SURFACE_HOVER = 0xFF222833;
			SURFACE_ACTIVE = 0xFF29303D;
			BORDER = 0xFF262C37;
			BORDER_STRONG = 0xFF343C4A;
			TEXT = 0xFFE6EAF0;
			TEXT_DIM = 0xFF97A2B3;
			TEXT_FAINT = 0xFF667185;
			BACKDROP = 0xB0080A0E;
		}
		HUD_BACKDROP = alpha(BG, 0.6f);
	}

	// Fonts (assets/exile/font/*.json, Iosevka Aile with vanilla fallback)
	public static final FontDescription UI = new FontDescription.Resource(Identifier.fromNamespaceAndPath("exile", "ui"));
	public static final FontDescription UI_BOLD = new FontDescription.Resource(Identifier.fromNamespaceAndPath("exile", "ui_bold"));

	/**
	 * A TrueType font is rasterised once, at a fixed resolution. Drawn at any
	 * other GUI scale its glyphs get resampled and strokes come out uneven, so
	 * each weight exists once per GUI scale (ui_x1..ui_x8) and text is drawn
	 * with the one that maps 1:1 onto screen pixels (see FontProviderMixin).
	 */
	private static final int MAX_SCALE = 8;
	private static final FontDescription[] UI_SCALED = scaled("ui");
	private static final FontDescription[] UI_BOLD_SCALED = scaled("ui_bold");

	private static FontDescription[] scaled(String name) {
		FontDescription[] out = new FontDescription[MAX_SCALE];
		for (int i = 0; i < MAX_SCALE; i++) {
			out[i] = new FontDescription.Resource(Identifier.fromNamespaceAndPath("exile", name + "_x" + (i + 1)));
		}
		return out;
	}

	/** The variant of an Exile font rasterised for the current GUI scale; other fonts unchanged. */
	public static FontDescription forGuiScale(FontDescription font) {
		boolean regular = font.equals(UI);
		if (!regular && !font.equals(UI_BOLD)) return font;
		Minecraft mc = Minecraft.getInstance();
		int scale = mc == null || mc.getWindow() == null ? 4 : Math.clamp(mc.getWindow().getGuiScale(), 1, MAX_SCALE);
		return (regular ? UI_SCALED : UI_BOLD_SCALED)[scale - 1];
	}

	public static MutableComponent text(String s) {
		return Component.literal(s).withStyle(st -> st.withFont(UI));
	}

	public static MutableComponent bold(String s) {
		return Component.literal(s).withStyle(st -> st.withFont(UI_BOLD));
	}

	/** Replaces the alpha of an ARGB colour (0..1). */
	public static int alpha(int argb, float a) {
		return (Math.round(Math.clamp(a, 0f, 1f) * 255) << 24) | (argb & 0xFFFFFF);
	}
}
