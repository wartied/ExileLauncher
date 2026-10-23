package dev.exile.client.mixin;

import dev.exile.client.perf.FrameStats;
import dev.exile.client.perf.FrameStats.Phase;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Frame Budget timing for the top-level phases of the main loop. */
@Mixin(Minecraft.class)
abstract class MinecraftMixin {
	@Inject(method = "runTick", at = @At("HEAD"))
	private void exile$gameStart(boolean advanceGameTime, CallbackInfo ci) {
		FrameStats.begin(Phase.GAME);
	}

	@Inject(method = "runTick", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/Minecraft;renderFrame(Z)V"))
	private void exile$gameEnd(boolean advanceGameTime, CallbackInfo ci) {
		FrameStats.end(Phase.GAME);
	}

	@Inject(method = "renderFrame", at = @At(value = "INVOKE", target = "Lcom/mojang/renderpearl/api/commands/CommandEncoder;submit()V"))
	private void exile$gpuStart(boolean advanceGameTime, CallbackInfo ci) {
		FrameStats.begin(Phase.GPU);
	}

	@Inject(method = "renderFrame", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/LevelRenderer;endFrame()V"))
	private void exile$gpuEnd(boolean advanceGameTime, CallbackInfo ci) {
		FrameStats.end(Phase.GPU);
	}

	@Inject(method = "renderFrame", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/FramerateLimiter;limitDisplayFPS(I)V"))
	private void exile$idleStart(boolean advanceGameTime, CallbackInfo ci) {
		FrameStats.begin(Phase.IDLE);
	}

	@Inject(method = "renderFrame", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/FramerateLimiter;limitDisplayFPS(I)V", shift = At.Shift.AFTER))
	private void exile$idleEnd(boolean advanceGameTime, CallbackInfo ci) {
		FrameStats.end(Phase.IDLE);
	}

	@Inject(method = "renderFrame", at = @At("RETURN"))
	private void exile$frameEnd(boolean advanceGameTime, CallbackInfo ci) {
		FrameStats.endFrame();
	}
}
