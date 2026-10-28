package dev.exile.client.mixin;

import dev.exile.client.config.ExileConfig;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Your own name tag: vanilla never draws a name over the camera's entity. With
 * the setting on, yours shows in third person (and with it your badges), unless
 * the HUD is hidden, you're invisible or you're carrying a passenger, the same
 * rules other players' tags follow.
 */
@Mixin(LivingEntityRenderer.class)
abstract class LivingEntityRendererMixin {
	@Inject(method = "shouldShowName", at = @At("RETURN"), cancellable = true)
	private void exile$ownNameTag(LivingEntity entity, double distanceToCameraSq, CallbackInfoReturnable<Boolean> cir) {
		if (cir.getReturnValueZ() || !ExileConfig.get().features.ownNameTag) return;
		Minecraft mc = Minecraft.getInstance();
		if (entity == mc.player && !mc.gui.hud.isHidden() && !entity.isInvisible() && !entity.isVehicle()) {
			cir.setReturnValue(true);
		}
	}
}
