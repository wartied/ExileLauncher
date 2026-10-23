package dev.exile.client.ui;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;

/**
 * Adds an "E" icon button to the title screen's row of small icon buttons
 * (friends, language, accessibility), re-centring the row, so the Exile menu
 * and mod list are reachable before joining a world.
 */
public final class TitleScreenButton {
	private TitleScreenButton() {}

	public static void init() {
		ScreenEvents.AFTER_INIT.register((client, screen, width, height) -> {
			if (!(screen instanceof TitleScreen)) return;
			List<AbstractWidget> widgets = Screens.getWidgets(screen);
			// The icon row: 20x20 buttons sharing one y, below the wide buttons.
			List<AbstractWidget> icons = new ArrayList<>(widgets.stream()
				.filter(w -> w.getWidth() == 20 && w.getHeight() == 20 && w.visible)
				.toList());
			if (icons.isEmpty()) return;
			int rowY = icons.stream().mapToInt(AbstractWidget::getY).max().getAsInt();
			icons.removeIf(w -> w.getY() != rowY);
			icons.sort(Comparator.comparingInt(AbstractWidget::getX));
			int gap = icons.size() > 1 ? icons.get(1).getX() - icons.get(0).getX() - 20 : 4;

			Button exile = Button.builder(Component.literal("E"), b -> client.gui.setScreen(new ExileMenuScreen(screen)))
				.bounds(icons.getLast().getX() + 20 + gap, rowY, 20, 20)
				.tooltip(Tooltip.create(Component.literal("Exile")))
				.build();
			// Shift the row left by half the added width so it stays centred.
			int shift = (20 + gap) / 2;
			for (AbstractWidget w : icons) w.setX(w.getX() - shift);
			exile.setX(exile.getX() - shift);
			widgets.add(exile);
		});
	}
}
