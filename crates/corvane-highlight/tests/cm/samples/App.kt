/*
 * Kotlin sample /* with a nested comment */ still a comment
 * Ünïcödé: Ärger, Ωmega
 */
@file:JvmName("AppKt")
package dev.corvane.sample

import kotlinx.coroutines.*
import java.util.concurrent.atomic.AtomicInteger as Counter

typealias Handler = (String) -> Unit

@Target(AnnotationTarget.CLASS)
annotation class Marker(val name: String = "")

sealed class Result<out T> {
	data class Ok<T>(val value: T) : Result<T>()
	data class Err(val error: Throwable) : Result<Nothing>()
	object Loading : Result<Nothing>()
}

enum class Color(val rgb: Int) { RED(0xFF0000), GREEN(0x00FF00), BLUE(0x0000FF) }

interface Greeter { fun greet(name: String): String }

@Marker("main")
open class App(private val name: String) : Greeter {
	companion object {
		const val MAX = 1_000_000L
		val HEX = 0xCAFE_BABE
		val BIN = 0b1010_1010
		val RATIO = 3.5e-3
		val HALF = .5f
		@JvmStatic fun create() = App("default")
	}

	private lateinit var counter: Counter
	var nullable: String? = null
	val chars = listOf('a', '\n', '\'', 'é')

	override fun greet(name: String): String = "Hello, $name! You are ${name.length} chars (${"nested"})"

	fun multiline(): String {
		val raw = """
			Raw string with "quotes" and $name and ${name.uppercase()}
			and a backslash \n that stays literal
		"""
		val trimmed = """single line triple""".trimIndent()
		val escaped = "tab\t\"quote\" \$notTemplate \\"
		return raw + trimmed + escaped
	}

	suspend fun load(): Result<String> = coroutineScope {
		val deferred = async { delay(10); "done" }
		try {
			Result.Ok(deferred.await())
		} catch (e: Exception) {
			Result.Err(e)
		} finally {
			println("finished")
		}
	}

	fun classify(x: Any?): String = when (x) {
		null -> "null"
		is String -> "string of ${x.length}"
		!is Number -> "other"
		in 1..10 -> "small"
		else -> "number"
	}

	inline fun <reified T> cast(value: Any): T? = value as? T

	fun operators(a: Int, b: Int): Int {
		var c = a * b + a / b - a % b
		c += 1; c -= 2; c *= 3
		val isBig = c > 10 && c <= 100 || !(c == 0)
		val list = listOf(1, 2, 3).map { it * 2 }.filter { it > 2 }
		val star = list.map(Int::toString)
		val elvis = nullable?.length ?: 0
		for (i in 0 until 10 step 2) { if (i == 4) continue else if (i == 8) break }
		do { c-- } while (c > 0)
		val bad = "unterminated string
		return this.hashCode() + c
	}
}

fun main(args: Array<String>) {
	val app = App.create()
	println(app.greet(args.firstOrNull() ?: "world"))
	val open = """never closed
	across lines with $interpolation
