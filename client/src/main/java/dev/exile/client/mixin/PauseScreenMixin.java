package dev.exile.client.mixin;

import com.llamalad7.mixinextras.sugar.Local;
import dev.exile.client.ui.ExileMenuScreen;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.layouts.GridLayout;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Adds the Exile button to the pause menu, as a full-width row above Save and Quit. */
@Mixin(PauseScreen.class)
abstract class PauseScreenMixin extends Screen {
	protected PauseScreenMixin(Component title) {
		super(title);
	}

	@Inject(
		method = "createPauseMenu",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/network/chat/CommonComponents;disconnectButtonLabel(Z)Lnet/minecraft/network/chat/Component;"
		)
	)
	private void exile$addButton(CallbackInfo ci, @Local GridLayout.RowHelper helper) {
		Screen self = this;
		helper.addChild(
			Button.builder(Component.literal("Exile"), b -> this.minecraft.gui.setScreen(new ExileMenuScreen(self))).width(204).build(),
			2
		);
	}
}
