package dev.exile.client.ui;

import com.terraformersmc.modmenu.api.ConfigScreenFactory;
import com.terraformersmc.modmenu.api.ModMenuApi;
import dev.exile.client.ExileClient;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.ModContainer;
import net.fabricmc.loader.api.entrypoint.EntrypointContainer;
import net.fabricmc.loader.api.metadata.CustomValue;
import net.fabricmc.loader.api.metadata.ModMetadata;

/**
 * The mod list for Exile's Mods tab: top-level mods only (libraries bundled
 * inside other mods and Fabric API's modules are folded into their parent),
 * each with its config screen if it offers one through the Mod Menu API.
 */
public final class ModEntries {
	public record Entry(String id, String name, String version, String description, ConfigScreenFactory<?> config) {}

	private ModEntries() {}

	public static List<Entry> load() {
		Map<String, ConfigScreenFactory<?>> own = new HashMap<>();
		Map<String, ConfigScreenFactory<?>> provided = new HashMap<>();
		for (EntrypointContainer<ModMenuApi> c : FabricLoader.getInstance().getEntrypointContainers("modmenu", ModMenuApi.class)) {
			String id = c.getProvider().getMetadata().getId();
			try {
				ModMenuApi api = c.getEntrypoint();
				if (overridesConfigFactory(api)) own.put(id, api.getModConfigScreenFactory());
				api.getProvidedConfigScreenFactories().forEach(provided::putIfAbsent);
			} catch (Throwable t) {
				// A broken integration in one mod must not take the list down.
				ExileClient.LOG.warn("Mod Menu integration of {} failed", id, t);
			}
		}

		boolean fabricApi = FabricLoader.getInstance().isModLoaded("fabric-api");
		List<Entry> out = new ArrayList<>();
		for (ModContainer mod : FabricLoader.getInstance().getAllMods()) {
			ModMetadata m = mod.getMetadata();
			String id = m.getId();
			if (id.equals("java") || mod.getContainingMod().isPresent() || hasParent(m)) continue;
			// Fabric API's modules are nested in its jar normally, but load as
			// separate jars in development: fold them into Fabric API either way.
			if (fabricApi && id.startsWith("fabric-") && !id.equals("fabric-api")) continue;
			if (id.equals("mixinextras")) continue;
			ConfigScreenFactory<?> factory = own.containsKey(m.getId()) ? own.get(m.getId()) : provided.get(m.getId());
			out.add(new Entry(m.getId(), m.getName(), m.getVersion().getFriendlyString(), m.getDescription(), factory));
		}
		out.sort(Comparator.comparing((Entry e) -> !e.id().equals("exile")).thenComparing(e -> e.name().toLowerCase()));
		return out;
	}

	/** Mod Menu's own default returns a "no screen" factory; only real overrides count. */
	private static boolean overridesConfigFactory(ModMenuApi api) {
		try {
			return api.getClass().getMethod("getModConfigScreenFactory").getDeclaringClass() != ModMenuApi.class;
		} catch (NoSuchMethodException e) {
			return false;
		}
	}

	private static boolean hasParent(ModMetadata m) {
		CustomValue mm = m.getCustomValue("modmenu");
		return mm != null && mm.getType() == CustomValue.CvType.OBJECT && mm.getAsObject().containsKey("parent");
	}
}
