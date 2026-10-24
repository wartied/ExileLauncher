package dev.exile.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import dev.exile.client.feature.Armour;
import java.util.function.Function;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.entity.ArmorModelSet;
import net.minecraft.client.renderer.entity.player.AvatarRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Slim armour: bake a slim-fit twin of the player's armour models with the same model factory. */
@Mixin(AvatarRenderer.class)
abstract class AvatarRendererMixin {
	@SuppressWarnings({"rawtypes", "unchecked"})
	@WrapOperation(
		method = "<init>",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/entity/ArmorModelSet;bake(Lnet/minecraft/client/renderer/entity/ArmorModelSet;Lnet/minecraft/client/model/geom/EntityModelSet;Ljava/util/function/Function;)Lnet/minecraft/client/renderer/entity/ArmorModelSet;"
		)
	)
	private ArmorModelSet exile$bakeSlimArmour(ArmorModelSet locations, EntityModelSet models, Function factory, Operation<ArmorModelSet> original) {
		ArmorModelSet normal = original.call(locations, models, factory);
		Armour.pair(normal, original.call(Armour.SLIM_PLAYER_ARMOR, models, factory));
		return normal;
	}
}
