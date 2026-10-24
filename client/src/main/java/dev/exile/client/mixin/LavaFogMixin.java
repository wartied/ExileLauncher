package dev.exile.client.mixin;

import dev.exile.client.feature.Visibility;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.fog.FogData;
import net.minecraft.client.renderer.fog.environment.LavaFogEnvironment;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Clear lava: lift the lava fog out to the render distance. */
@Mixin(LavaFogEnvironment.class)
abstract class LavaFogMixin {
	@Inject(method = "setupFog", at = @At("TAIL"))
	private void exile$clearLava(FogData fog, Camera camera, ClientLevel level, float renderDistance, DeltaTracker delta, CallbackInfo ci) {
		if (Visibility.clearLava()) Visibility.clear(fog, renderDistance);
	}
}
