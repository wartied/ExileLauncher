package dev.exile.client.bench;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import dev.exile.client.ExileClient;
import dev.exile.client.perf.FrameStats;
import dev.exile.client.perf.Tuning;
import java.io.IOException;
import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.Arrays;
import java.util.function.Consumer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.InactivityFpsLimit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.presets.WorldPresets;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.phys.Vec3;

/**
 * Exile Bench: a repeatable benchmark.
 *
 * <p>Flies the camera around a fixed circle in a fixed-seed world with fixed
 * settings (render distance, FOV, time of day, weather, no mobs), one warm-up
 * lap to load and mesh chunks, then one measured lap. Every frame time of the
 * measured lap is recorded, so results include averages and stutter (1% and
 * 0.1% lows). Results are written as JSON to {@code <game dir>/exile/bench}.
 *
 * <p>Started from the Exile menu, or automatically with {@code -Dexile.bench=auto}
 * (the game quits when done; used for A/B testing optimisations).
 */
public final class ExileBench {
	public static final String WORLD = "Exile Bench";
	/** Fixed seed: same terrain for everyone, so runs are comparable across machines. */
	public static final long SEED = 7_041_986_245L;
	private static final int LAP_TICKS = 600; // 30 s per lap
	private static final double RADIUS = 96.0;
	private static final double ALTITUDE = 110.0;
	private static final float PITCH = 18.0f;
	private static final int RENDER_DISTANCE = 12;
	private static final int MAX_FRAMES = 200_000; // plenty for 30 s at very high FPS
	/**
	 * Scene: "terrain" (default: a loaded circle, measures steady-state rendering),
	 * "entities" (the circle plus a ring of still animals), or "travel" (a straight
	 * 600-block flight into new terrain, measured from the start: chunk loading,
	 * meshing and lighting as players feel them while exploring).
	 */
	public static final String SCENE = System.getProperty("exile.bench.scene", "terrain");
	private static final int SCENE_ENTITIES = 400;
	private static final boolean TRAVEL = SCENE.equals("travel");
	/** Travel speed: 20 blocks per second, about elytra cruising speed. */
	private static final double TRAVEL_BLOCKS_PER_TICK = 1.0;

	public enum State { IDLE, LEAVING, LOADING, SETTLING, WARMUP, MEASURE, DONE }

	private static State state = State.IDLE;
	private static int stateTicks;
	private static double centerX, centerZ;
	private static boolean autoQuit;
	private static Consumer<BenchResult> onDone;
	private static Saved saved;

	// Measured lap data (preallocated; nothing allocates per frame).
	private static final long[] frames = new long[MAX_FRAMES];
	private static int frameCount;
	private static final long[] phaseTotals = new long[FrameStats.phases().length];
	private static long gcCountStart, gcTimeStart;
	private static long allocStart, renderAllocStart, measureStartNs;
	private static long liveHeapMb;
	private static final com.sun.management.ThreadMXBean THREADS =
		(com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();

	private ExileBench() {}

	public static State state() {
		return state;
	}

	public static boolean running() {
		return state != State.IDLE && state != State.DONE;
	}

	/** 0..1 progress across warm-up and measurement. */
	public static float progress() {
		return switch (state) {
			case WARMUP -> stateTicks / (float) (LAP_TICKS * 2);
			case MEASURE -> TRAVEL ? stateTicks / (float) LAP_TICKS : 0.5f + stateTicks / (float) (LAP_TICKS * 2);
			case DONE -> 1f;
			default -> 0f;
		};
	}

	public static void start(boolean quitWhenDone, Consumer<BenchResult> done) {
		if (running()) return;
		Minecraft mc = Minecraft.getInstance();
		autoQuit = quitWhenDone;
		onDone = done;
		saved = Saved.capture(mc);
		applyBenchSettings(mc);
		stateTicks = 0;
		ExileClient.LOG.info("Exile Bench: starting ({})", Tuning.describe());
		if (mc.level != null) {
			// Already in a world: leave it first and open the bench world once
			// the old one has fully closed (see tick).
			state = State.LEAVING;
			mc.disconnectWithProgressScreen();
		} else {
			state = State.LOADING;
			openBenchWorld(mc);
		}
	}

	private static void openBenchWorld(Minecraft mc) {
		if (mc.getLevelSource().levelExists(WORLD)) {
			mc.createWorldOpenFlows().openWorld(WORLD, () -> abort("Couldn't open the bench world"));
		} else {
			LevelSettings settings = new LevelSettings(
				WORLD, GameType.SPECTATOR, LevelSettings.DifficultySettings.DEFAULT, true, WorldDataConfiguration.DEFAULT
			);
			mc.createWorldOpenFlows().createFreshLevel(
				WORLD, settings, new WorldOptions(SEED, true, false), WorldPresets::createNormalWorldDimensions, null
			);
		}
	}

	public static void abort(String reason) {
		ExileClient.LOG.warn("Exile Bench aborted: {}", reason);
		finish(null);
	}

	/** Called at the end of every client tick. */
	public static void tick(Minecraft mc) {
		if (!running()) return;
		// Keep vanilla's AFK frame cap from kicking in: the bench has no input.
		mc.getFramerateLimitTracker().onInputReceived();
		stateTicks++;
		LocalPlayer player = mc.player;

		switch (state) {
			case LEAVING -> {
				if (mc.level == null && mc.getSingleplayerServer() == null) {
					setState(State.LOADING);
					openBenchWorld(mc);
				}
			}
			case LOADING -> {
				if (player != null && mc.level != null && mc.getSingleplayerServer() != null) {
					centerX = Math.floor(player.getX()) + 0.5;
					centerZ = Math.floor(player.getZ()) + 0.5;
					prepareWorld(mc.getSingleplayerServer(), centerX, centerZ);
					setState(State.SETTLING);
				} else if (stateTicks > 20 * 120) {
					abort("world took too long to load");
				}
			}
			case SETTLING -> {
				fly(player, 0);
				if (stateTicks >= 40) {
					// Travel measures from the first block: loading is the point.
					if (TRAVEL) {
						beginMeasuring();
						setState(State.MEASURE);
					} else {
						setState(State.WARMUP);
					}
				}
			}
			case WARMUP -> {
				fly(player, stateTicks);
				if (stateTicks >= LAP_TICKS) {
					beginMeasuring();
					setState(State.MEASURE);
				}
			}
			case MEASURE -> {
				fly(player, TRAVEL ? stateTicks : LAP_TICKS + stateTicks);
				if (stateTicks >= LAP_TICKS) {
					FrameStats.setListener(null);
					finish(buildResult(mc));
				}
			}
			default -> {}
		}
	}

	private static void setState(State s) {
		state = s;
		stateTicks = 0;
	}

	/** Places the camera on the circle. Called after the player's own tick, so
	 * vanilla interpolates smoothly between positions. */
	private static void fly(LocalPlayer player, int tick) {
		if (player == null) return;
		if (TRAVEL) {
			// Straight line along +X, looking ahead and slightly down.
			player.setDeltaMovement(Vec3.ZERO);
			player.setPos(centerX + tick * TRAVEL_BLOCKS_PER_TICK, ALTITUDE, centerZ);
			player.setYRot(-90f);
			player.setXRot(PITCH);
			return;
		}
		double angle = (tick % LAP_TICKS) / (double) LAP_TICKS * Math.PI * 2;
		double x = centerX + Math.cos(angle) * RADIUS;
		double z = centerZ + Math.sin(angle) * RADIUS;
		// Face along the direction of travel (tangent to the circle).
		float yaw = (float) Math.toDegrees(angle) + 180f;
		player.setDeltaMovement(Vec3.ZERO);
		player.setPos(x, ALTITUDE, z);
		player.setYRot(yaw);
		player.setXRot(PITCH);
	}

	private static void prepareWorld(MinecraftServer server, double cx, double cz) {
		server.execute(() -> {
			var source = server.createCommandSourceStack().withSuppressedOutput();
			for (String cmd : new String[] {
				"gamerule advance_time false", "gamerule advance_weather false", "gamerule spawn_mobs false",
				"time set 6000", "weather clear", "gamemode spectator @a",
				// Same world state every run: no leftover animals or items.
				"kill @e[type=!player]",
			}) {
				server.getCommands().performPrefixedCommand(source, cmd);
			}
			if (SCENE.equals("entities")) spawnEntityRing(server, source, cx, cz);
		});
	}

	/** Places still, silent, invulnerable animals evenly around the ring the camera flies. */
	private static void spawnEntityRing(MinecraftServer server, net.minecraft.commands.CommandSourceStack source, double cx, double cz) {
		String[] types = {"cow", "sheep", "pig", "chicken"};
		var level = server.overworld();
		for (int i = 0; i < SCENE_ENTITIES; i++) {
			double a = i / (double) SCENE_ENTITIES * Math.PI * 2;
			// Two lanes, slightly inside and outside the path.
			double r = RADIUS + (i % 2 == 0 ? -6 : 6);
			int x = (int) Math.floor(cx + Math.cos(a) * r);
			int z = (int) Math.floor(cz + Math.sin(a) * r);
			int y = level.getHeight(net.minecraft.world.level.levelgen.Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z);
			server.getCommands().performPrefixedCommand(source, String.format(
				"summon minecraft:%s %d %d %d {NoAI:1b,Silent:1b,Invulnerable:1b,PersistenceRequired:1b}", types[i % types.length], x, y, z
			));
		}
	}

	private static void beginMeasuring() {
		// Settle the heap once before measuring and record what's still live:
		// the memory the game actually needs, comparable between runs.
		System.gc();
		liveHeapMb = (Runtime.getRuntime().totalMemory() - Runtime.getRuntime().freeMemory()) / (1024 * 1024);
		frameCount = 0;
		Arrays.fill(phaseTotals, 0L);
		long[] gc = gcTotals();
		gcCountStart = gc[0];
		gcTimeStart = gc[1];
		// Bytes allocated by every thread, and by the render thread alone (this one).
		allocStart = THREADS.getTotalThreadAllocatedBytes();
		renderAllocStart = THREADS.getCurrentThreadAllocatedBytes();
		measureStartNs = System.nanoTime();
		FrameStats.setListener((frameNanos, phaseNanos) -> {
			if (frameCount < MAX_FRAMES) frames[frameCount++] = frameNanos;
			for (int i = 0; i < phaseTotals.length; i++) phaseTotals[i] += phaseNanos[i];
		});
	}

	private static long[] gcTotals() {
		long count = 0, time = 0;
		for (GarbageCollectorMXBean gc : ManagementFactory.getGarbageCollectorMXBeans()) {
			count += Math.max(0, gc.getCollectionCount());
			time += Math.max(0, gc.getCollectionTime());
		}
		return new long[] {count, time};
	}

	private static BenchResult buildResult(Minecraft mc) {
		long[] gc = gcTotals();
		double seconds = (System.nanoTime() - measureStartNs) / 1e9;
		double allocMbS = (THREADS.getTotalThreadAllocatedBytes() - allocStart) / 1e6 / seconds;
		double renderAllocMbS = (THREADS.getCurrentThreadAllocatedBytes() - renderAllocStart) / 1e6 / seconds;
		return BenchResult.compute(
			Arrays.copyOf(frames, frameCount), phaseTotals.clone(), gc[0] - gcCountStart, gc[1] - gcTimeStart,
			allocMbS, renderAllocMbS, liveHeapMb, mc
		);
	}

	private static void finish(BenchResult result) {
		FrameStats.setListener(null);
		Minecraft mc = Minecraft.getInstance();
		if (saved != null) {
			saved.restore(mc);
			saved = null;
		}
		state = result == null ? State.IDLE : State.DONE;
		if (result != null) {
			Path file = write(mc, result);
			ExileClient.LOG.info("Exile Bench: {} -> {}", result.summary(), file);
		}
		Consumer<BenchResult> done = onDone;
		onDone = null;
		if (autoQuit) {
			mc.stop();
		} else if (done != null) {
			done.accept(result);
		}
	}

	private static Path write(Minecraft mc, BenchResult result) {
		Path dir = mc.gameDirectory.toPath().resolve("exile").resolve("bench");
		String name = LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyy-MM-dd_HH-mm-ss"));
		String label = System.getProperty("exile.bench.label", "");
		if (!label.isBlank()) name += "_" + label.replaceAll("[^A-Za-z0-9._-]", "_");
		Path file = dir.resolve(name + ".json");
		try {
			Files.createDirectories(dir);
			Gson gson = new GsonBuilder().setPrettyPrinting().create();
			Files.writeString(file, gson.toJson(result.toJson()), StandardCharsets.UTF_8);
		} catch (IOException e) {
			ExileClient.LOG.warn("Exile Bench: couldn't save results", e);
		}
		return file;
	}

	private static void applyBenchSettings(Minecraft mc) {
		var o = mc.options;
		o.renderDistance().set(RENDER_DISTANCE);
		o.simulationDistance().set(8);
		o.fov().set(70);
		o.enableVsync().set(false);
		o.framerateLimit().set(260); // 260 = unlimited
		o.inactivityFpsLimit().set(InactivityFpsLimit.MINIMIZED);
		o.pauseOnLostFocus = false;
		if (!mc.gui.hud.isHidden()) mc.gui.hud.toggle();
	}

	/** The user's settings, put back after the run. */
	private record Saved(
		int renderDistance, int simulationDistance, int fov, boolean vsync, int fpsLimit,
		InactivityFpsLimit inactivity, boolean pauseOnLostFocus, boolean hudHidden
	) {
		static Saved capture(Minecraft mc) {
			var o = mc.options;
			return new Saved(
				o.renderDistance().get(), o.simulationDistance().get(), o.fov().get(), o.enableVsync().get(),
				o.framerateLimit().get(), o.inactivityFpsLimit().get(), o.pauseOnLostFocus, mc.gui.hud.isHidden()
			);
		}

		void restore(Minecraft mc) {
			var o = mc.options;
			o.renderDistance().set(renderDistance);
			o.simulationDistance().set(simulationDistance);
			o.fov().set(fov);
			o.enableVsync().set(vsync);
			o.framerateLimit().set(fpsLimit);
			o.inactivityFpsLimit().set(inactivity);
			o.pauseOnLostFocus = pauseOnLostFocus;
			if (mc.gui.hud.isHidden() != hudHidden) mc.gui.hud.toggle();
		}
	}

	/** Mods loaded, for the result file. */
	static JsonArray modList() {
		JsonArray arr = new JsonArray();
		FabricLoader.getInstance().getAllMods().stream()
			.map(m -> m.getMetadata())
			.filter(m -> !m.getId().startsWith("fabric-") && !m.getId().equals("java"))
			.forEach(m -> arr.add(m.getId() + " " + m.getVersion().getFriendlyString()));
		return arr;
	}

	static JsonObject tuningJson() {
		return Tuning.toJson();
	}
}
