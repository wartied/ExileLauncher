package com.terraformersmc.modmenu.api;

/** Update channels, least to most stable. Exile has no update checker, so the preference is RELEASE. */
public enum UpdateChannel {
	ALPHA,
	BETA,
	RELEASE;

	public static UpdateChannel getUserPreference() {
		return RELEASE;
	}
}
