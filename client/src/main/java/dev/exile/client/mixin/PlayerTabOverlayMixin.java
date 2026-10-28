package dev.exile.client.mixin;

import dev.exile.client.badges.Badges;
import net.minecraft.client.gui.components.PlayerTabOverlay;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Badges after names in the player list (Tab). The list sizes its columns from this too. */
@Mixin(PlayerTabOverlay.class)
abstract class PlayerTabOverlayMixin {
	@Inject(method = "getNameForDisplay", at = @At("RETURN"), cancellable = true)
	private void exile$badges(PlayerInfo info, CallbackInfoReturnable<Component> cir) {
		cir.setReturnValue(Badges.withBadges(info.getProfile().id(), cir.getReturnValue()));
	}
}
