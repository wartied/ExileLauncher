package dev.exile.client.perf;

import dev.exile.client.ExileClient;

/** Registers every optimisation's switch at startup, so the Performance tab and bench results list them all. */
public final class Optimisations {
	private Optimisations() {}

	public static void init() {
		Tuning.Switch[] all = {BackgroundThrottle.SWITCH};
		ExileClient.LOG.info("Exile optimisations: {}", Tuning.describe());
		assert all.length == Tuning.all().size();
	}
}
