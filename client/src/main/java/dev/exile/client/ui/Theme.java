package dev.exile.client.ui;

import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.resources.Identifier;

/**
 * Exile's look, shared with the launcher: one dark palette derived from the
 * brand blue #678FC5, flat surfaces, Iosevka Aile text.
 */
public final class Theme {
	private Theme() {}

	// Colours (ARGB)
	public static final int BACKDROP = 0xB0080A0E;
	public static final int BG = 0xFF111419;
	public static final int SIDEBAR = 0xFF151920;
	public static final int SURFACE = 0xFF1B2029;
	public static final int SURFACE_HOVER = 0xFF222833;
	public static final int SURFACE_ACTIVE = 0xFF29303D;
	public static final int BORDER = 0xFF262C37;
	public static final int BORDER_STRONG = 0xFF343C4A;
	public static final int TEXT = 0xFFE6EAF0;
	public static final int TEXT_DIM = 0xFF97A2B3;
	public static final int TEXT_FAINT = 0xFF667185;
	public static final int ACCENT = 0xFF678FC5;
	public static final int ACCENT_LIGHT = 0xFF8EAEDB;
	public static final int ACCENT_DIM = 0xFF2B3E5A;
	public static final int BUTTON = 0xFF4F79B2;
	public static final int BUTTON_HOVER = 0xFF5A84BD;
	public static final int BUTTON_PRESSED = 0xFF466CA0;
	public static final int DANGER = 0xFFE5534B;
	public static final int WARN = 0xFFD9A23B;
	/** HUD element backdrop: dark and translucent so the world stays visible. */
	public static final int HUD_BACKDROP = 0x99111419;

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
