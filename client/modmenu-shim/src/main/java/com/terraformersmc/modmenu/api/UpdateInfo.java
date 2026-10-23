package com.terraformersmc.modmenu.api;

import net.minecraft.network.chat.Component;

/** Present for API compatibility; Exile does not run update checks. */
public interface UpdateInfo {
	boolean isUpdateAvailable();

	default Component getUpdateMessage() {
		return null;
	}

	String getDownloadLink();

	UpdateChannel getUpdateChannel();
}
