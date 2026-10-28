package dev.exile.client.badges;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.authlib.properties.Property;
import com.mojang.authlib.services.ProfileResult;
import dev.exile.client.ExileClient;
import dev.exile.client.config.ExileConfig;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.security.Signature;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicBoolean;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;
import net.minecraft.world.entity.player.ProfileKeyPair;
import net.minecraft.world.entity.player.ProfilePublicKey;

/**
 * Exile badges next to player names, on name tags and in the player list.
 *
 * <p>Who has which badge lives on exiles.dev, not in this mod: Exile's team
 * hands out the manual ones there, and Early Supporter goes to everyone who
 * first used Exile (launcher or client) before 11 April 2027. So the client signs in
 * to the badge service once per game, then looks up the players it actually
 * sees, in batches, and remembers the answers
 * for a while. Players choose which of their badges show in the launcher.
 */
public final class Badges {
	private static final String API = System.getProperty("exile.api", "https://exiles.dev/api");
	/** How long an answer is used before asking again, and how soon to retry after a failure. */
	private static final long FRESH_MS = 15 * 60_000L;
	private static final long RETRY_MS = 2 * 60_000L;
	private static final int BATCH = 100;

	/** Badges keep their own colours whatever the name around them is styled like. */
	private static final Style STYLE = Style.EMPTY
		.withColor(0xFFFFFF)
		.withBold(false)
		.withItalic(false)
		.withUnderlined(false)
		.withStrikethrough(false)
		.withObfuscated(false);
	/**
	 * The badges are glyphs in a small bitmap font. Unlike an inline sprite,
	 * which Minecraft draws a unit higher than the text, a font glyph can be
	 * placed exactly: these span the text's line from its top to one unit below
	 * the baseline, the middle of a name tag's bar.
	 */
	private static final FontDescription FONT = new FontDescription.Resource(Identifier.fromNamespaceAndPath("exile", "badges"));
	private static final Map<String, String> GLYPHS = Map.of(
		"exile", "\uE000",
		"dev", "\uE001",
		"ogu", "\uE002",
		"early_supporter", "\uE003"
	);
	/** Glyphs already leave 1 unit after themselves; this adds 1 more between badges (a normal space is 4). */
	private static final FontDescription GAP = new FontDescription.Resource(Identifier.fromNamespaceAndPath("exile", "badge_gap"));

	private record Entry(List<String> badges, long checked) {}

	private static final Map<UUID, Entry> known = new ConcurrentHashMap<>();
	private static final Set<UUID> wanted = ConcurrentHashMap.newKeySet();
	private static final AtomicBoolean busy = new AtomicBoolean();
	private static final HttpClient http = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(10)).build();
	private static boolean sessionStarted;
	private static int ticks;

	private Badges() {}

	/** {@code name} followed by the player's badges, or just {@code name}. */
	public static Component withBadges(UUID player, Component name) {
		if (name == null || player == null || !ExileConfig.get().features.badges) return name;
		List<String> list = badgesOf(player);
		if (list.isEmpty()) return name;
		MutableComponent badges = Component.empty().withStyle(STYLE).append(" ");
		for (int i = 0; i < list.size(); i++) {
			if (i > 0) badges.append(Component.literal(" ").withStyle(s -> s.withFont(GAP)));
			badges.append(Component.literal(GLYPHS.get(list.get(i))).withStyle(s -> s.withFont(FONT)));
		}
		return name.copy().append(badges);
	}

	private static List<String> badgesOf(UUID player) {
		Entry e = known.get(player);
		if (e == null || System.currentTimeMillis() - e.checked > FRESH_MS) wanted.add(player);
		return e == null ? List.of() : e.badges;
	}

	/** Once a second: sign in once per game, and look up players seen since the last batch. */
	public static void tick(Minecraft mc) {
		if (++ticks % 20 != 0 || !ExileConfig.get().features.badges) return;
		if (!sessionStarted) {
			sessionStarted = true;
			CompletableFuture.runAsync(() -> startSession(mc), Util.nonCriticalIoPool());
		}
		if (wanted.isEmpty() || !busy.compareAndSet(false, true)) return;

		List<UUID> batch = new ArrayList<>();
		long now = System.currentTimeMillis();
		for (Iterator<UUID> it = wanted.iterator(); it.hasNext() && batch.size() < BATCH; ) {
			UUID id = it.next();
			it.remove();
			batch.add(id);
			// Counts as checked while the answer is on its way, so it isn't asked for twice.
			Entry old = known.get(id);
			known.put(id, new Entry(old == null ? List.of() : old.badges, now));
		}
		JsonArray uuids = new JsonArray();
		batch.forEach(id -> uuids.add(id.toString()));
		JsonObject body = new JsonObject();
		body.add("uuids", uuids);
		post("/badges", body).whenComplete((res, err) -> {
			try {
				if (err != null || res == null) {
					retryLater(batch);
					return;
				}
				JsonObject all = res.getAsJsonObject("badges");
				long at = System.currentTimeMillis();
				for (UUID id : batch) {
					JsonElement list = all == null ? null : all.get(undashed(id));
					known.put(id, new Entry(list == null ? List.of() : names(list.getAsJsonArray()), at));
				}
			} catch (RuntimeException e) {
				retryLater(batch);
			} finally {
				busy.set(false);
			}
		});
	}

	private static void retryLater(List<UUID> batch) {
		long at = System.currentTimeMillis() - FRESH_MS + RETRY_MS;
		for (UUID id : batch) {
			Entry old = known.get(id);
			known.put(id, new Entry(old == null ? List.of() : old.badges, at));
		}
	}

	/**
	 * Signs in to the badge service, which marks this account as an Exile user
	 * (Early Supporter) and returns its badges straight away.
	 *
	 * <p>Mojang refuses requests from the service's host, so the service can't
	 * ask Mojang who this is. The account proves it with what Mojang has already
	 * signed: its chat key (the one Minecraft signs chat with) and its profile,
	 * UUID and name. The service's one-time id is signed with the chat key here,
	 * and the service checks all three signatures against Mojang's public keys.
	 * The login token and the private key stay in this game.
	 */
	private static void startSession(Minecraft mc) {
		try {
			var user = mc.getUser();
			ProfileKeyPair keys = mc.getProfileKeyPairManager().prepareKeyPair().join()
				.orElseThrow(() -> new IllegalStateException("no chat key (an offline account?)"));
			ProfilePublicKey.Data key = keys.publicKey().data();
			ProfileResult result = mc.services().sessionService().fetchProfile(user.getProfileId(), true);
			Property textures = result == null ? null : result.profile().properties().get("textures").stream()
				.filter(Property::hasSignature)
				.findFirst()
				.orElse(null);
			if (textures == null) throw new IllegalStateException("no signed profile from Mojang");

			JsonObject start = post("/session/start", new JsonObject()).join();
			String serverId = start.get("serverId").getAsString();
			Signature signer = Signature.getInstance("SHA256withRSA");
			signer.initSign(keys.privateKey());
			signer.update(serverId.getBytes(StandardCharsets.UTF_8));

			Base64.Encoder base64 = Base64.getEncoder();
			JsonObject profile = new JsonObject();
			profile.addProperty("value", textures.value());
			profile.addProperty("signature", textures.signature());
			JsonObject publicKey = new JsonObject();
			publicKey.addProperty("publicKey", base64.encodeToString(key.key().getEncoded()));
			publicKey.addProperty("expiresAt", key.expiresAt().toString());
			publicKey.addProperty("mojangSignature", base64.encodeToString(key.keySignature()));
			JsonObject finish = new JsonObject();
			finish.addProperty("serverId", serverId);
			finish.add("profile", profile);
			finish.add("key", publicKey);
			finish.addProperty("signature", base64.encodeToString(signer.sign()));
			JsonObject me = post("/session/finish", finish).join();
			List<String> shown = new ArrayList<>(names(me.getAsJsonArray("badges")));
			shown.removeAll(names(me.getAsJsonArray("hidden")));
			known.put(user.getProfileId(), new Entry(List.copyOf(shown), System.currentTimeMillis()));
		} catch (Exception e) {
			// Offline, an offline-mode account, or the service is down: badges just
			// don't show for this account until the next start.
			ExileClient.LOG.debug("Badge sign-in skipped: {}", e.toString());
		}
	}

	private static CompletableFuture<JsonObject> post(String path, JsonObject body) {
		HttpRequest request = HttpRequest.newBuilder(URI.create(API + path))
			.timeout(Duration.ofSeconds(15))
			.header("Content-Type", "application/json")
			.header("User-Agent", "ExileClient")
			.POST(HttpRequest.BodyPublishers.ofString(body.toString()))
			.build();
		return http.sendAsync(request, HttpResponse.BodyHandlers.ofString()).thenApply(res -> {
			if (res.statusCode() != 200) throw new IllegalStateException("HTTP " + res.statusCode());
			return JsonParser.parseString(res.body()).getAsJsonObject();
		});
	}

	/** Badge names this client has art for; anything newer is left out. */
	private static List<String> names(JsonArray array) {
		List<String> out = new ArrayList<>();
		if (array == null) return out;
		for (JsonElement e : array) {
			String name = e.getAsString();
			if (GLYPHS.containsKey(name)) out.add(name);
		}
		return List.copyOf(out);
	}

	private static String undashed(UUID id) {
		return id.toString().replace("-", "").toLowerCase(Locale.ROOT);
	}
}
