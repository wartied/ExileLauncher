package dev.exile.client.perf;

import com.google.gson.JsonObject;
import dev.exile.client.config.ExileConfig;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Switches for every Exile optimisation.
 *
 * <p>Each one follows the player's setting (Performance tab) unless forced
 * with {@code -Dexile.opt.<name>=true|false}; {@code -Dexile.opt.all=false}
 * turns them all off. Exile Bench uses the overrides to measure each
 * technique on its own, and records the active set in every result file.
 */
public final class Tuning {
	private static final List<Switch> SWITCHES = new ArrayList<>();

	private Tuning() {}

	public static Switch register(String name, String title, String description, boolean defaultOn) {
		Switch s = new Switch(name, title, description, defaultOn);
		SWITCHES.add(s);
		return s;
	}

	public static List<Switch> all() {
		return Collections.unmodifiableList(SWITCHES);
	}

	public static final class Switch {
		public final String name;
		public final String title;
		public final String description;
		private final boolean defaultOn;
		private final Boolean forced;

		Switch(String name, String title, String description, boolean defaultOn) {
			this.name = name;
			this.title = title;
			this.description = description;
			this.defaultOn = defaultOn;
			String own = System.getProperty("exile.opt." + name);
			String all = System.getProperty("exile.opt.all");
			this.forced = own != null ? Boolean.valueOf(own) : all != null ? Boolean.valueOf(all) : null;
		}

		public boolean enabled() {
			return forced != null ? forced : ExileConfig.get().optimisation(name, defaultOn);
		}

		/** Forced by a -D property for testing; the switch can't be changed in-game. */
		public boolean forced() {
			return forced != null;
		}

		public void set(boolean on) {
			ExileConfig.get().performance.optimisations.put(name, on);
			ExileConfig.get().save();
		}
	}

	public static String describe() {
		StringBuilder sb = new StringBuilder();
		for (Switch s : SWITCHES) sb.append(sb.isEmpty() ? "" : ", ").append(s.name).append('=').append(s.enabled() ? "on" : "off");
		return sb.isEmpty() ? "no optimisations registered" : sb.toString();
	}

	public static JsonObject toJson() {
		JsonObject o = new JsonObject();
		for (Switch s : SWITCHES) o.addProperty(s.name, s.enabled());
		return o;
	}
}
