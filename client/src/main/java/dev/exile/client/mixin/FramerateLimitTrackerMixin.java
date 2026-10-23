package dev.exile.client.mixin;

import com.mojang.blaze3d.platform.FramerateLimitTracker;
import dev.exile.client.perf.BackgroundThrottle;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Background frame cap: see {@link BackgroundThrottle}. */
@Mixin(FramerateLimitTracker.class)
abstract class FramerateLimitTrackerMixin {
	@Shadow @Final private Minecraft minecraft;

	@Inject(method = "getFramerateLimit", at = @At("RETURN"), cancellable = true)
	private void exile$backgroundCap(CallbackInfoReturnable<Integer> cir) {
		int vanilla = cir.getReturnValueI();
		int limit = BackgroundThrottle.limit(minecraft, vanilla);
		if (limit != vanilla) cir.setReturnValue(limit);
	}
}
