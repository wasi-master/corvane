/*
 * Copyright (c) Corvane contributors. Licensed MIT.
 */
package dev.corvane.sample;

import java.util.*;
import java.util.function.Function;
import static java.lang.Math.max;

/**
 * Javadoc with {@link Map} and <b>markup</b>, and @param tags.
 * Ünïcödé: Grüße, 你好.
 */
@SuppressWarnings({"unchecked", "rawtypes"})
public final class Main<T extends Comparable<? super T>> implements Runnable, AutoCloseable {
	private static final long SERIAL = 0x7FFF_FFFFL;
	private static final int MILLION = 1_000_000, BIN = 0b1010_1010, OCT = 0777;
	protected volatile boolean running = true;
	transient double ratio = 3.14e-2d, half = .5, big = 1_000.5e1_0f;
	char c = 'x', nl = '\n', q = '\'', u = 'é';
	String s = "hello \"world\"\t\\", empty = "";
	var inferred = new ArrayList<Map<String, List<Integer>>>();
	Object[] items = new Object[] { null, true, false };

	@Override
	public void run() {
		String block = """
			Text block with "quotes" and \""" escaped triple.
			Second line: ${not interpolated}
			""";
		String afterBlock = "done";
		int[] arr = {1, 2, 3};
		for (int i = 0; i < arr.length; i++) {
			if (arr[i] % 2 == 0) continue;
			else if (arr[i] > 2) break;
		}
		switch (arr.length) {
			case 1 -> System.out.println("one");
			case 2, 3 -> { System.out.println("few"); }
			default -> throw new IllegalStateException("many: " + arr.length);
		}
		Function<Integer, Integer> twice = x -> x * 2;
		Runnable r = () -> System.out.println(twice.apply(21));
		items.forEach(System.out::println);
		synchronized (this) {
			running = !running && ratio >= 0 || ratio <= 1;
		}
		assert running : "must run";
		Object o = inferred instanceof List<?> l ? l.size() : -1;
		label:
		do { ratio /= 2; } while (ratio > 0.1);
		String open = "unterminated
		int after = 1;
	}

	public static <K, V extends Number> Map<K, V> merge(Map<K, V> a, Map<K, V> b) throws Exception {
		try (var in = new java.io.FileInputStream("x")) {
			return a;
		} catch (IOException | RuntimeException e) {
			throw e;
		} finally {
			System.gc();
		}
	}

	@Deprecated(since = "9", forRemoval = true)
	native void legacy();

	public @interface Marker {
		String value() default "";
	}

	enum Mode { FAST, SLOW; Mode next() { return this == FAST ? SLOW : FAST; } }

	record Pair<A, B>(A first, B second) {}

	sealed interface Shape permits Circle, Square {}

	@Override public void close() {}

	strictfp double compute(double x) { return Math.sqrt(x) * SERIAL; }
}
