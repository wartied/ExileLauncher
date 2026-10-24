package dev.exile.client.mixin;

import dev.exile.client.feature.Freelook;
import dev.exile.client.feature.Zoom;
import net.minecraft.client.Camera;
import org.objectweb.asm.Opcodes;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Free look: apply Exile's camera angles just before vanilla places a
 * detached (third-person) camera, so the orbit and wall clipping work as normal.
 * Zoom: narrow the world's field of view (the hand has its own, untouched).
 */
@Mixin(Camera.class)
abstract class CameraMixin {
	@Shadow
	protected abstract void setRotation(float yRot, float xRot);

	@Inject(
		method = "alignWithEntity",
		at = @At(value = "FIELD", target = "Lnet/minecraft/client/Camera;detached:Z", opcode = Opcodes.PUTFIELD)
	)
	private void exile$freelookRotation(float partialTicks, CallbackInfo ci) {
		if (Freelook.active()) setRotation(Freelook.yaw(), Freelook.pitch());
	}

	@Inject(method = "calculateFov", at = @At("RETURN"), cancellable = true)
	private void exile$zoom(float partialTicks, CallbackInfoReturnable<Float> cir) {
		float scale = Zoom.fovScale();
		if (scale != 1f) cir.setReturnValue(cir.getReturnValueF() * scale);
	}
}
