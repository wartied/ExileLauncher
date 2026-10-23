package dev.exile.client.mixin;

import dev.exile.client.feature.Freelook;
import net.minecraft.client.Camera;
import org.objectweb.asm.Opcodes;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Free look: apply Exile's camera angles just before vanilla places a
 * detached (third-person) camera, so the orbit and wall clipping work as normal.
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
}
