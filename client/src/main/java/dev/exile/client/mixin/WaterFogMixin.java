package dev.exile.client.mixin;

import dev.exile.client.feature.Visibility;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.fog.FogData;
import net.minecraft.client.renderer.fog.environment.WaterFogEnvironment;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Clear water: lift the underwater fog out to the render distance. */
@Mixin(WaterFogEnvironment.class)
abstract class WaterFogMixin {
	@Inject(method = "setupFog", at = @At("TAIL"))
	private void exile$clearWater(FogData fog, Camera camera, ClientLevel level, float renderDistance, DeltaTracker delta, CallbackInfo ci) {
		if (Visibility.clearWater()) Visibility.clear(fog, renderDistance);
	}
}
