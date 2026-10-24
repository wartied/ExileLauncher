package dev.exile.client.mixin;

import dev.exile.client.feature.ClickCounter;
import dev.exile.client.feature.Zoom;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;
import net.minecraft.client.input.MouseButtonInfo;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Counts clicks for the CPS counter and keystrokes (in-game only, not in
 * menus). Zoom: slower mouse while zoomed, and the wheel changes the zoom.
 */
@Mixin(MouseHandler.class)
abstract class MouseHandlerMixin {
	@Inject(method = "onButton", at = @At("HEAD"))
	private void exile$countClick(long handle, MouseButtonInfo info, int action, CallbackInfo ci) {
		if (action == 1 && Minecraft.getInstance().gui.screen() == null) ClickCounter.press(info.button());
	}

	@ModifyArg(method = "turnPlayer", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/player/LocalPlayer;turn(DD)V"), index = 0)
	private double exile$zoomTurnX(double x) {
		return x * Zoom.sensitivityScale();
	}

	@ModifyArg(method = "turnPlayer", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/player/LocalPlayer;turn(DD)V"), index = 1)
	private double exile$zoomTurnY(double y) {
		return y * Zoom.sensitivityScale();
	}

	@Inject(method = "onScroll", at = @At("HEAD"), cancellable = true)
	private void exile$zoomScroll(long handle, double xoffset, double yoffset, CallbackInfo ci) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.gui.screen() == null && mc.gui.overlay() == null && mc.player != null && Zoom.scroll(yoffset)) ci.cancel();
	}
}
