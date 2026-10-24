package dev.exile.client.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import dev.exile.client.feature.Armour;
import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.ArmorModelSet;
import net.minecraft.client.renderer.entity.layers.HumanoidArmorLayer;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import net.minecraft.world.entity.EquipmentSlot;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Invisible armour (your own player only) and slim armour (players), see {@link Armour}. */
@Mixin(HumanoidArmorLayer.class)
abstract class HumanoidArmorLayerMixin {
	@Shadow @Final private ArmorModelSet<?> modelSet;

	@Inject(
		method = "submit(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;ILnet/minecraft/client/renderer/entity/state/HumanoidRenderState;FF)V",
		at = @At("HEAD"),
		cancellable = true
	)
	private void exile$invisibleArmour(PoseStack poseStack, SubmitNodeCollector collector, int light, HumanoidRenderState state, float yRot, float xRot, CallbackInfo ci) {
		if (Armour.hide(state)) ci.cancel();
	}

	@Inject(method = "getArmorModel", at = @At("HEAD"), cancellable = true)
	private void exile$slimArmour(HumanoidRenderState state, EquipmentSlot slot, CallbackInfoReturnable<HumanoidModel<?>> cir) {
		if (state.isBaby) return;
		ArmorModelSet<?> slim = Armour.slimFor(modelSet);
		if (slim != null) cir.setReturnValue((HumanoidModel<?>) slim.get(slot));
	}
}
