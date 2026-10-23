package dev.exile.client.mixin;

import java.util.BitSet;
import net.minecraft.client.gui.components.toasts.ToastManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Which toast slots (32 px rows down the top-right corner) are in use, so the HUD can keep clear of them. */
@Mixin(ToastManager.class)
public interface ToastManagerAccessor {
	@Accessor("occupiedSlots")
	BitSet exile$occupiedSlots();
}
