package com.terraformersmc.modmenu.api;

/** Present for API compatibility; Exile does not run update checks. */
public interface UpdateChecker {
	UpdateInfo checkForUpdates();
}
