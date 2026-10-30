package dev.exile.client.ui;

import java.util.Locale;
import java.util.function.BooleanSupplier;
import java.util.function.Consumer;
import java.util.function.Supplier;

/**
 * One feature, HUD element or optimisation as a compact card in the Exile
 * menu: its name, a one-line summary and (usually) a switch. Cards with more
 * settings than the switch open a detail page. Search matches the same cards,
 * so everything that can be changed can also be found.
 */
final class Card {
	/** Draws a card's detail page from {@code y}; returns where it ended. */
	@FunctionalInterface
	interface Detail {
		int draw(ExileMenuScreen s, int x, int y, int w);
	}

	final ExileMenuScreen.Tab tab;
	final String section;
	final String title;
	final String description;
	private String keywords = "";
	BooleanSupplier on;
	Consumer<Boolean> set;
	/** Set when the switch can't be changed (forced by launch options). */
	BooleanSupplier locked = () -> false;
	/** Shown under the title; the description when null. */
	Supplier<String> summary;
	Detail detail;
	/** Run on click instead of opening a detail page (e.g. the layout editor). */
	Consumer<ExileMenuScreen> action;
	/** Whether {@link #action} can run now. */
	BooleanSupplier actionEnabled = () -> true;

	private Card(ExileMenuScreen.Tab tab, String section, String title, String description) {
		this.tab = tab;
		this.section = section;
		this.title = title;
		this.description = description;
	}

	static Card of(ExileMenuScreen.Tab tab, String section, String title, String description) {
		return new Card(tab, section, title, description);
	}

	Card toggle(BooleanSupplier on, Consumer<Boolean> set) {
		this.on = on;
		this.set = set;
		return this;
	}

	Card locked(BooleanSupplier locked) {
		this.locked = locked;
		return this;
	}

	Card summary(Supplier<String> summary) {
		this.summary = summary;
		return this;
	}

	Card detail(Detail detail) {
		this.detail = detail;
		return this;
	}

	Card action(Consumer<ExileMenuScreen> action, BooleanSupplier enabled) {
		this.action = action;
		this.actionEnabled = enabled;
		return this;
	}

	/** Extra words search should find this card by. */
	Card keywords(String words) {
		this.keywords = words;
		return this;
	}

	boolean hasToggle() {
		return on != null;
	}

	String summaryText() {
		return summary != null ? summary.get() : description;
	}

	/** Every word of the query appears in the card's title, description, section, tab or keywords. */
	boolean matches(String query) {
		String haystack = (title + " " + description + " " + section + " " + tab.title + " " + keywords).toLowerCase(Locale.ROOT);
		for (String word : query.toLowerCase(Locale.ROOT).trim().split("\\s+")) {
			if (!word.isEmpty() && !haystack.contains(word)) return false;
		}
		return true;
	}
}
