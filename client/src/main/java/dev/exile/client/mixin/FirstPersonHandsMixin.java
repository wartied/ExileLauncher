package dev.exile.client.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import dev.exile.client.feature.Visibility;
import net.minecraft.client.renderer.FirstPersonHandsAndItemsRenderer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.state.level.FirstPersonHandsAndItemsRenderState;
import net.minecraft.client.renderer.state.level.PlayerRenderState;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ShieldItem;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Low shield: a held shield sits lower in first person, idle or blocking. */
@Mixin(FirstPersonHandsAndItemsRenderer.class)
abstract class FirstPersonHandsMixin {
	@Inject(
		method = "submitArmWithItem",
		at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;pushPose()V", shift = At.Shift.AFTER)
	)
	private void exile$lowShield(
		PlayerRenderState playerState,
		FirstPersonHandsAndItemsRenderState state,
		float partialTicks,
		float xRot,
		InteractionHand hand,
		float attack,
		ItemStack itemStack,
		float inverseArmHeight,
		PoseStack poseStack,
		SubmitNodeCollector collector,
		int light,
		CallbackInfo ci
	) {
		if (Visibility.lowShield() && itemStack.getItem() instanceof ShieldItem) {
			poseStack.translate(0f, -Visibility.LOW_SHIELD_OFFSET, 0f);
		}
	}
}
