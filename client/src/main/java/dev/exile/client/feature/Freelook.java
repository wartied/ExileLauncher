package dev.exile.client.feature;

import dev.exile.client.config.ExileConfig;
import net.minecraft.client.CameraType;
import net.minecraft.client.Minecraft;
import net.minecraft.util.Mth;

/**
 * Free look: while the key is held, the mouse orbits the camera around the
 * player instead of turning them. Aim, movement and what the server sees stay
 * exactly as they were; releasing the key snaps the view back.
 */
public final class Freelook {
	private static boolean active;
	private static float yaw, pitch;
	private static CameraType previous;

	private Freelook() {}

	public static boolean active() {
		return active;
	}

	public static float yaw() {
		return yaw;
	}

	public static float pitch() {
		return pitch;
	}

	/** Called every client tick with the key's state. */
	public static void tick(Minecraft mc, boolean held) {
		boolean want = held && ExileConfig.get().features.freelook && mc.player != null && mc.gui.screen() == null;
		if (want && !active) {
			active = true;
			yaw = mc.player.getYRot();
			pitch = mc.player.getXRot();
			previous = mc.options.getCameraType();
			if (previous.isFirstPerson()) mc.options.setCameraType(CameraType.THIRD_PERSON_BACK);
		} else if (!want && active) {
			active = false;
			if (previous != null) mc.options.setCameraType(previous);
			previous = null;
		}
	}

	/** Takes a mouse turn for the camera. Returns true if it was consumed. */
	public static boolean consumeTurn(double xo, double yo) {
		if (!active) return false;
		// Same sensitivity scale vanilla applies in Entity.turn.
		yaw += (float) xo * 0.15f;
		pitch = Mth.clamp(pitch + (float) yo * 0.15f, -90f, 90f);
		return true;
	}
}
