package dev.exile.client.feature;

import dev.exile.client.config.ExileConfig;
import java.util.Collections;
import java.util.Map;
import java.util.WeakHashMap;
import net.fabricmc.fabric.api.client.rendering.v1.ModelLayerRegistry;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.geom.ModelLayerLocation;
import net.minecraft.client.model.geom.builders.CubeDeformation;
import net.minecraft.client.model.geom.builders.LayerDefinition;
import net.minecraft.client.model.geom.builders.MeshDefinition;
import net.minecraft.client.model.player.PlayerModel;
import net.minecraft.client.renderer.entity.ArmorModelSet;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import net.minecraft.resources.Identifier;

/**
 * Slim armour and invisible armour. Both only change what this client draws.
 *
 * <p>Vanilla armour is the body model with every cube inflated outward (1.0
 * pixel for helmet, chestplate and boots, 0.5 for leggings), which is what
 * makes it look chunky. Slim armour bakes a second set of player armour
 * models with the least inflation that still clears the skin's outer layer
 * (hat 0.5, jacket/sleeves/pants 0.25) and keeps the pieces stacking in the
 * right order, so nothing flickers through:
 *
 * <pre>
 *   helmet 1.0 -> 0.6   chestplate 1.0 -> 0.5   leggings 0.5 -> 0.45   boots 0.9 -> 0.4
 * </pre>
 *
 * Both sets exist side by side, so switching is instant and needs no reload.
 */
public final class Armour {
	public static final ArmorModelSet<ModelLayerLocation> SLIM_PLAYER_ARMOR = new ArmorModelSet<>(
		layer("helmet"), layer("chestplate"), layer("leggings"), layer("boots")
	);
	/** Each player renderer's normal armour models, paired with their slim twins. */
	private static final Map<ArmorModelSet<?>, ArmorModelSet<?>> SLIM = Collections.synchronizedMap(new WeakHashMap<>());

	private Armour() {}

	private static ModelLayerLocation layer(String name) {
		return new ModelLayerLocation(Identifier.fromNamespaceAndPath("exile", "player_armor_slim"), name);
	}

	public static void init() {
		ModelLayerRegistry.registerArmorModelLayers(SLIM_PLAYER_ARMOR, Armour::slimDefinitions);
	}

	private static ArmorModelSet<LayerDefinition> slimDefinitions() {
		// One outer inflation is shared by helmet, chestplate and boots in
		// vanilla's builder, so build twice and take the helmet from the second.
		ArmorModelSet<MeshDefinition> body = PlayerModel.createArmorMeshSet(new CubeDeformation(0.45f), new CubeDeformation(0.5f));
		ArmorModelSet<MeshDefinition> head = PlayerModel.createArmorMeshSet(new CubeDeformation(0.45f), new CubeDeformation(0.6f));
		return new ArmorModelSet<>(head.head(), body.chest(), body.legs(), body.feet()).map(mesh -> LayerDefinition.create(mesh, 64, 32));
	}

	public static void pair(ArmorModelSet<?> normal, ArmorModelSet<?> slim) {
		SLIM.put(normal, slim);
	}

	/** The slim twin of a player renderer's armour models, or null when slim armour is off (or for mobs). */
	public static ArmorModelSet<?> slimFor(ArmorModelSet<?> normal) {
		return ExileConfig.get().features.slimArmour ? SLIM.get(normal) : null;
	}

	/** True when this is your own player and invisible armour is on. */
	public static boolean hide(HumanoidRenderState state) {
		if (!ExileConfig.get().features.invisibleArmour || !(state instanceof AvatarRenderState avatar)) return false;
		var player = Minecraft.getInstance().player;
		return player != null && avatar.id == player.getId();
	}
}
