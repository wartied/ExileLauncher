package dev.exile.client.mixin;

import dev.exile.client.ui.Theme;
import net.minecraft.network.chat.FontDescription;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Draws Exile's fonts with the variant rasterised for the current GUI scale.
 * Swapping the font here, before the provider's one-entry cache, means text
 * built at any time switches over the moment the GUI scale changes.
 */
@Mixin(targets = "net.minecraft.client.gui.font.FontManager$CachedFontProvider")
abstract class FontProviderMixin {
	@ModifyVariable(method = "glyphs", at = @At("HEAD"), argsOnly = true)
	private FontDescription exile$scaledFont(FontDescription description) {
		return Theme.forGuiScale(description);
	}
}
