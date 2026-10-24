package dev.exile.client.mixin;

import dev.exile.client.feature.Visibility;
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import net.minecraft.client.renderer.state.LightmapRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Fullbright. The lightmap shader lifts every light level to at least the
 * night vision colour times its strength; with a white colour, full
 * strength and full brightness, every level comes out fully lit. Only the
 * values sent to the shader change: the player's brightness setting is left
 * alone, so turning fullbright off restores it.
 */
@Mixin(LightmapRenderStateExtractor.class)
abstract class LightmapMixin {
	@Inject(method = "extract", at = @At("RETURN"))
	private void exile$fullbright(LightmapRenderState state, float partialTicks, CallbackInfo ci) {
		if (!Visibility.fullbright()) return;
		state.nightVisionEffectIntensity = 1f;
		state.nightVisionColor = LightmapRenderStateExtractor.WHITE;
		state.brightness = 1f;
	}
}
