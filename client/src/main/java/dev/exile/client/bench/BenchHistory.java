package dev.exile.client.bench;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;
import net.minecraft.client.Minecraft;

/** Past bench results, read from the JSON files Exile Bench writes. */
public final class BenchHistory {
	public record Row(String when, String label, double avgFps, double low1, double low01, String gpu) {}

	private BenchHistory() {}

	public static Path dir() {
		return Minecraft.getInstance().gameDirectory.toPath().resolve("exile").resolve("bench");
	}

	/** Newest first. */
	public static List<Row> recent(int max) {
		List<Row> rows = new ArrayList<>();
		if (!Files.isDirectory(dir())) return rows;
		try (Stream<Path> files = Files.list(dir())) {
			for (Path f : files.filter(p -> p.toString().endsWith(".json")).sorted(Comparator.reverseOrder()).limit(max).toList()) {
				try {
					JsonObject o = JsonParser.parseString(Files.readString(f, StandardCharsets.UTF_8)).getAsJsonObject();
					String name = f.getFileName().toString();
					String when = name.length() >= 16 ? name.substring(0, 10) + " " + name.substring(11, 16).replace('-', ':') : name;
					String gpu = o.has("environment") ? o.getAsJsonObject("environment").get("gpu").getAsString() : "";
					rows.add(new Row(
						when, o.has("label") ? o.get("label").getAsString() : "",
						o.get("avg_fps").getAsDouble(), o.get("low_1pct_fps").getAsDouble(), o.get("low_0_1pct_fps").getAsDouble(), gpu
					));
				} catch (IOException | RuntimeException ignored) {
					// Skip files that aren't bench results.
				}
			}
		} catch (IOException ignored) {
			// Missing or unreadable folder: no history.
		}
		return rows;
	}
}
