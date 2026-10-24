package dev.exile.client.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import dev.exile.client.feature.Visibility;
import net.minecraft.client.renderer.ScreenEffectRenderer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.state.level.PlayerRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Low fire (the burning overlay moves down) and clear water (no underwater
 * texture overlay).
 *
 * <p>The flames are placed inside the drawing callback, which runs later
 * than {@code submitFire} itself, so the offset goes into the two
 * {@code translate} calls in that callback rather than onto the pose stack.
 */
@Mixin(ScreenEffectRenderer.class)
abstract class ScreenEffectRendererMixin {
	@ModifyArg(
		method = "lambda$submitFire$0",
		at = @At(value = "INVOKE", target = "Lorg/joml/Matrix4f;translate(FFF)Lorg/joml/Matrix4f;"),
		index = 1
	)
	private static float exile$lowFire(float y) {
		return Visibility.lowFire() ? y - Visibility.LOW_FIRE_OFFSET : y;
	}

	@Inject(method = "submitWater", at = @At("HEAD"), cancellable = true)
	private static void exile$clearWaterOverlay(PlayerRenderState.WaterOverlay overlay, PoseStack poseStack, SubmitNodeCollector collector, CallbackInfo ci) {
		if (Visibility.clearWater()) ci.cancel();
	}
}
