package dev.exile.client.mixin;

import com.llamalad7.mixinextras.sugar.Local;
import dev.exile.client.ui.MenuTheme;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Menu themes (see {@link MenuTheme}): every GUI sprite and texture, and
 * every line of text, passes through these three methods.
 */
@Mixin(GuiGraphicsExtractor.class)
abstract class GuiGraphicsMixin {
	@ModifyVariable(
		method = "blitSprite(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIIII)V",
		at = @At("HEAD"),
		argsOnly = true,
		ordinal = 4
	)
	private int exile$themeSprite(int color, @Local(argsOnly = true) Identifier location) {
		return MenuTheme.sprite(location, color);
	}

	@ModifyVariable(
		method = "blit(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIFFIIIIIII)V",
		at = @At("HEAD"),
		argsOnly = true,
		ordinal = 8
	)
	private int exile$themeTexture(int color, @Local(argsOnly = true) Identifier texture) {
		return MenuTheme.texture(texture, color);
	}

	@ModifyVariable(
		method = "text(Lnet/minecraft/client/gui/Font;Lnet/minecraft/util/FormattedCharSequence;IIIZ)V",
		at = @At("HEAD"),
		argsOnly = true,
		ordinal = 2
	)
	private int exile$themeText(int color) {
		return MenuTheme.text(color);
	}
}
