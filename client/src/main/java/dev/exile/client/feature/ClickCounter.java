package dev.exile.client.feature;

import com.mojang.blaze3d.platform.InputConstants;

/** Counts mouse clicks per second for the CPS and keystrokes elements. */
public final class ClickCounter {
	private static final int CAPACITY = 64; // > any humanly possible clicks per second
	private static final long[] left = new long[CAPACITY];
	private static final long[] right = new long[CAPACITY];
	private static int leftHead, rightHead;

	private ClickCounter() {}

	/** Called on press, outside screens. Buttons use SDL numbering (see InputConstants). */
	public static void press(int button) {
		long now = System.nanoTime();
		if (button == InputConstants.MOUSE_BUTTON_LEFT) {
			left[leftHead] = now;
			leftHead = (leftHead + 1) % CAPACITY;
		} else if (button == InputConstants.MOUSE_BUTTON_RIGHT) {
			right[rightHead] = now;
			rightHead = (rightHead + 1) % CAPACITY;
		}
	}

	public static int leftCps() {
		return count(left);
	}

	public static int rightCps() {
		return count(right);
	}

	private static int count(long[] clicks) {
		long cutoff = System.nanoTime() - 1_000_000_000L;
		int n = 0;
		for (long t : clicks) if (t != 0 && t > cutoff) n++;
		return n;
	}
}
