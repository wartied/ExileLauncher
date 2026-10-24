package dev.exile.client.feature;

import dev.exile.client.config.ExileConfig;
import net.minecraft.client.renderer.fog.FogData;

/**
 * Seeing better, all client-side: clear water and lava, low fire and shield,
 * and fullbright. The hooks live in the mixins; the switches live here.
 */
public final class Visibility {
	/** How far down the fire overlay and the first-person shield move. */
	public static final float LOW_FIRE_OFFSET = 0.15f;
	public static final float LOW_SHIELD_OFFSET = 0.2f;

	private Visibility() {}

	private static ExileConfig.Features cfg() {
		return ExileConfig.get().features;
	}

	public static boolean clearWater() {
		return cfg().clearWater;
	}

	public static boolean clearLava() {
		return cfg().clearLava;
	}

	public static boolean lowFire() {
		return cfg().lowFire;
	}

	public static boolean lowShield() {
		return cfg().lowShield;
	}

	public static boolean fullbright() {
		return cfg().fullbright;
	}

	/**
	 * No liquid fog: only the usual fade at the edge of the render distance
	 * remains, as in open air.
	 */
	public static void clear(FogData fog, float renderDistance) {
		fog.environmentalStart = renderDistance * 4;
		fog.environmentalEnd = renderDistance * 4;
		fog.skyEnd = renderDistance;
		fog.cloudEnd = renderDistance;
	}
}
