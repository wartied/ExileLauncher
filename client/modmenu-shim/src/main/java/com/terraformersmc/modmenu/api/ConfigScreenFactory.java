package com.terraformersmc.modmenu.api;

import net.minecraft.client.gui.screens.Screen;

/** Builds a mod's configuration screen. */
@FunctionalInterface
public interface ConfigScreenFactory<S extends Screen> {
	S create(Screen parent);
}
