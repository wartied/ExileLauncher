package dev.exile.client.mixin;

import dev.exile.client.badges.Badges;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Badges after players' names on their name tags. */
@Mixin(EntityRenderer.class)
abstract class EntityRendererMixin {
	@Inject(method = "getNameTag", at = @At("RETURN"), cancellable = true)
	private void exile$badges(Entity entity, CallbackInfoReturnable<Component> cir) {
		if (entity instanceof Player player && cir.getReturnValue() != null) {
			cir.setReturnValue(Badges.withBadges(player.getUUID(), cir.getReturnValue()));
		}
	}
}
