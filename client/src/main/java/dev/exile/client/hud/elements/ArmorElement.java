package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/** Worn armour and held item, with durability coloured as it runs low. */
public final class ArmorElement extends HudElement {
	private static final EquipmentSlot[] SLOTS = {
		EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET, EquipmentSlot.MAINHAND
	};
	private static final int ROW = 17;
	/** Layout-editor placeholder; created on first use because items aren't registered at mod init. */
	private static ItemStack placeholder;
	private final ItemStack[] stacks = new ItemStack[SLOTS.length];
	private final Component[] labels = new Component[SLOTS.length];
	private final int[] lastDamage = new int[SLOTS.length];
	private int rows;
	private int textWidth;

	public ArmorElement() {
		super("armor", "Armor status", "Worn armour and held item with durability", true, 1f, 0.62f);
	}

	@Override
	public void tick(Minecraft mc) {
		var p = mc.player;
		rows = 0;
		textWidth = 0;
		for (EquipmentSlot slot : SLOTS) {
			ItemStack s = p.getItemBySlot(slot);
			if (s.isEmpty() || (slot == EquipmentSlot.MAINHAND && !s.isDamageableItem())) continue;
			int i = rows++;
			if (stacks[i] != s || lastDamage[i] != s.getDamageValue() || labels[i] == null) {
				stacks[i] = s;
				lastDamage[i] = s.getDamageValue();
				labels[i] = label(s);
			}
			textWidth = Math.max(textWidth, mc.font.width(labels[i]));
		}
	}

	private static Component label(ItemStack s) {
		if (!s.isDamageableItem()) return Theme.text("");
		int max = s.getMaxDamage();
		int left = max - s.getDamageValue();
		return Labels.line().append(Labels.colored(Integer.toString(left), HudDraw.levelColor(left / (float) max))).append(Labels.dim(" / " + max));
	}

	@Override
	public boolean hasContent(Minecraft mc) {
		return mc.player != null && rows > 0;
	}

	@Override
	public int width() {
		return rows == 0 ? 72 : 18 + textWidth;
	}

	@Override
	public int height() {
		return Math.max(1, rows) * ROW - 1;
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		if (rows == 0 && preview) {
			if (placeholder == null) placeholder = new ItemStack(Items.IRON_CHESTPLATE);
			d.g.item(placeholder, x, y);
			d.text(Labels.dim("240 / 240"), x + 18, y + 4, Theme.TEXT);
			return;
		}
		for (int i = 0; i < rows; i++) {
			int ry = y + i * ROW;
			d.g.item(stacks[i], x, ry);
			d.text(labels[i], x + 18, ry + 4, Theme.TEXT);
		}
	}
}
