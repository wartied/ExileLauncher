package dev.exile.client.hud.elements;

import dev.exile.client.ui.Theme;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;

/** "value unit" labels: value in the main colour, unit dimmed. */
final class Labels {
	private Labels() {}

	static MutableComponent valueUnit(String value, String unit) {
		return line().append(Theme.bold(value)).append(dim(unit));
	}

	/**
	 * Neutral root for a multi-part label. Appended parts inherit their
	 * parent's style, so parts are always appended to this, never to each other.
	 */
	static MutableComponent line() {
		return Component.empty();
	}

	static MutableComponent dim(String s) {
		return Component.literal(s).withStyle(st -> st.withFont(Theme.UI).withColor(Theme.TEXT_DIM & 0xFFFFFF));
	}

	static MutableComponent colored(String s, int argb) {
		return Component.literal(s).withStyle(st -> st.withFont(Theme.UI_BOLD).withColor(argb & 0xFFFFFF));
	}

	/** Ticks as m:ss (or h:mm:ss). */
	static String duration(int ticks) {
		int s = ticks / 20;
		return s >= 3600 ? String.format("%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60) : String.format("%d:%02d", s / 60, s % 60);
	}

	static String roman(int n) {
		return switch (n) {
			case 1 -> "I";
			case 2 -> "II";
			case 3 -> "III";
			case 4 -> "IV";
			case 5 -> "V";
			case 6 -> "VI";
			case 7 -> "VII";
			case 8 -> "VIII";
			case 9 -> "IX";
			case 10 -> "X";
			default -> Integer.toString(n);
		};
	}
}
