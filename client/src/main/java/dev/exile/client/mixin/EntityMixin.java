package dev.exile.client.mixin;

import dev.exile.client.feature.Freelook;
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Free look: mouse turns go to the camera, not the player. */
@Mixin(Entity.class)
abstract class EntityMixin {
	@Inject(method = "turn", at = @At("HEAD"), cancellable = true)
	private void exile$freelookTurn(double xo, double yo, CallbackInfo ci) {
		if ((Object) this == Minecraft.getInstance().player && Freelook.consumeTurn(xo, yo)) ci.cancel();
	}
}
