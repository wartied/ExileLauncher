package dev.exile.client.mixin;

import dev.exile.client.perf.FrameStats;
import dev.exile.client.perf.FrameStats.Phase;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Frame Budget timing for world/HUD preparation and drawing. */
@Mixin(GameRenderer.class)
abstract class GameRendererMixin {
	private static final String LEVEL_EXTRACT =
		"Lnet/minecraft/client/renderer/extract/LevelExtractor;extract(Lnet/minecraft/client/DeltaTracker;Lnet/minecraft/client/Camera;F)V";
	private static final String GUI_EXTRACT = "Lnet/minecraft/client/gui/Gui;extractRenderState(Lnet/minecraft/client/DeltaTracker;ZZ)V";
	private static final String RENDER_LEVEL = "Lnet/minecraft/client/renderer/GameRenderer;renderLevel()V";
	private static final String GUI_RENDER = "Lnet/minecraft/client/gui/render/GuiRenderer;render()V";

	@Inject(method = "extract", at = @At(value = "INVOKE", target = LEVEL_EXTRACT))
	private void exile$worldPrepStart(CallbackInfo ci) {
		FrameStats.begin(Phase.WORLD_PREP);
	}

	@Inject(method = "extract", at = @At(value = "INVOKE", target = LEVEL_EXTRACT, shift = At.Shift.AFTER))
	private void exile$worldPrepEnd(CallbackInfo ci) {
		FrameStats.end(Phase.WORLD_PREP);
	}

	@Inject(method = "extract", at = @At(value = "INVOKE", target = GUI_EXTRACT))
	private void exile$hudPrepStart(CallbackInfo ci) {
		FrameStats.begin(Phase.HUD_PREP);
	}

	@Inject(method = "extract", at = @At(value = "INVOKE", target = GUI_EXTRACT, shift = At.Shift.AFTER))
	private void exile$hudPrepEnd(CallbackInfo ci) {
		FrameStats.end(Phase.HUD_PREP);
	}

	@Inject(method = "render", at = @At(value = "INVOKE", target = RENDER_LEVEL))
	private void exile$worldDrawStart(CallbackInfo ci) {
		FrameStats.begin(Phase.WORLD_DRAW);
	}

	@Inject(method = "render", at = @At(value = "INVOKE", target = RENDER_LEVEL, shift = At.Shift.AFTER))
	private void exile$worldDrawEnd(CallbackInfo ci) {
		FrameStats.end(Phase.WORLD_DRAW);
	}

	@Inject(method = "render", at = @At(value = "INVOKE", target = GUI_RENDER))
	private void exile$hudDrawStart(CallbackInfo ci) {
		FrameStats.begin(Phase.HUD_DRAW);
	}

	@Inject(method = "render", at = @At(value = "INVOKE", target = GUI_RENDER, shift = At.Shift.AFTER))
	private void exile$hudDrawEnd(CallbackInfo ci) {
		FrameStats.end(Phase.HUD_DRAW);
	}
}
