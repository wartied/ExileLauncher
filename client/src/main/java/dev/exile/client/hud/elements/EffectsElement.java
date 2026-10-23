package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Theme;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.world.effect.MobEffectInstance;

/** Active potion effects: icon, name with level, and time left. */
public final class EffectsElement extends HudElement {
	private static final int ROW = 12;
	private final List<Row> rows = new ArrayList<>();
	private int widest;
	private int ticks;

	private record Row(Identifier sprite, Component name, Component time, boolean ending) {}

	public EffectsElement() {
		super("effects", "Potion effects", "Active effects with time remaining", true, 1f, 0f);
	}

	@Override
	public void tick(Minecraft mc) {
		// Times only change once a second.
		if (ticks++ % 20 != 0 && rows.size() == mc.player.getActiveEffects().size()) return;
		rows.clear();
		widest = 0;
		for (MobEffectInstance e : mc.player.getActiveEffects()) {
			if (!e.showIcon()) continue;
			String name = e.getEffect().value().getDisplayName().getString();
			if (e.getAmplifier() > 0) name += " " + Labels.roman(e.getAmplifier() + 1);
			Component time = Labels.dim(e.isInfiniteDuration() ? "∞" : Labels.duration(e.getDuration()));
			Component label = Theme.text(name);
			rows.add(new Row(Hud.getMobEffectSprite(e.getEffect()), label, time, !e.isInfiniteDuration() && e.endsWithin(200)));
			widest = Math.max(widest, mc.font.width(label) + 6 + mc.font.width(time));
		}
	}

	@Override
	public boolean hasContent(Minecraft mc) {
		return mc.player != null && !rows.isEmpty();
	}

	@Override
	public int width() {
		return rows.isEmpty() ? 90 : 12 + widest;
	}

	@Override
	public int height() {
		return Math.max(1, rows.size()) * ROW - 3;
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		if (rows.isEmpty() && preview) {
			d.text(Labels.line().append(Theme.text("Speed II  ")).append(Labels.dim("1:30")), x + 12, y, Theme.TEXT);
			return;
		}
		for (int i = 0; i < rows.size(); i++) {
			Row r = rows.get(i);
			int ry = y + i * ROW;
			d.g.blitSprite(RenderPipelines.GUI_TEXTURED, r.sprite, x, ry - 1, 9, 9);
			d.text(r.name, x + 12, ry, r.ending ? Theme.WARN : Theme.TEXT);
			d.text(r.time, x + 12 + d.width(r.name) + 6, ry, Theme.TEXT);
		}
	}
}
