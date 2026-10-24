package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Theme;
import java.util.List;
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
	/** Text-only rows are one line tall. */
	private static final int TEXT_ROW = 11;
	private static final String[] NAMES = {"Head", "Chest", "Legs", "Feet", "Hand"};
	/** Room for the longest slot name plus a gap. */
	private static final int NAME_WIDTH = 34;
	/** Layout-editor placeholder; created on first use because items aren't registered at mod init. */
	private static ItemStack placeholder;
	private final ItemStack[] stacks = new ItemStack[SLOTS.length];
	private final Component[] labels = new Component[SLOTS.length];
	/** Which slot each row shows (rows skip empty slots). */
	private final int[] slotOf = new int[SLOTS.length];
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
		for (int k = 0; k < SLOTS.length; k++) {
			EquipmentSlot slot = SLOTS[k];
			ItemStack s = p.getItemBySlot(slot);
			if (s.isEmpty() || (slot == EquipmentSlot.MAINHAND && !s.isDamageableItem())) continue;
			int i = rows++;
			slotOf[i] = k;
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
	public List<Mode> modes() {
		return List.of(Mode.FULL, Mode.ICON, Mode.TEXT);
	}

	@Override
	public int width() {
		return switch (mode()) {
			case ICON -> 16;
			case TEXT -> (rows == 0 ? 50 : textWidth) + NAME_WIDTH;
			case FULL -> rows == 0 ? 72 : 18 + textWidth;
		};
	}

	@Override
	public int height() {
		return Math.max(1, rows) * (mode() == Mode.TEXT ? TEXT_ROW : ROW) - 1;
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		Mode mode = mode();
		if (rows == 0 && preview) {
			if (placeholder == null) placeholder = new ItemStack(Items.IRON_CHESTPLATE);
			switch (mode) {
				case ICON -> d.g.item(placeholder, x, y);
				case TEXT -> {
					d.text(Labels.dim(NAMES[1]), x, y, Theme.TEXT);
					d.text(Labels.dim("240 / 240"), x + NAME_WIDTH, y, Theme.TEXT);
				}
				case FULL -> {
					d.g.item(placeholder, x, y);
					d.text(Labels.dim("240 / 240"), x + 18, y + 4, Theme.TEXT);
				}
			}
			return;
		}
		for (int i = 0; i < rows; i++) {
			switch (mode) {
				case ICON -> {
					int ry = y + i * ROW;
					d.g.item(stacks[i], x, ry);
					// Vanilla's durability bar under the item, instead of numbers.
					d.g.itemDecorations(Minecraft.getInstance().font, stacks[i], x, ry);
				}
				case TEXT -> {
					int ry = y + i * TEXT_ROW;
					d.text(Labels.dim(NAMES[slotOf[i]]), x, ry, Theme.TEXT);
					d.text(labels[i], x + NAME_WIDTH, ry, Theme.TEXT);
				}
				case FULL -> {
					int ry = y + i * ROW;
					d.g.item(stacks[i], x, ry);
					d.text(labels[i], x + 18, ry + 4, Theme.TEXT);
				}
			}
		}
	}
}
