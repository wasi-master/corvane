#!/usr/bin/env kotlin
// Summarise a CSV of expenses by category.
@file:Suppress("UNUSED_VARIABLE")

import java.io.File
import kotlin.math.roundToInt

enum class Category { FOOD, TRAVEL, OFFICE, OTHER }

data class Expense(val date: String, val category: Category, val cents: Long) {
    val euros: Double get() = cents / 100.0
}

sealed interface ParseResult {
    data class Ok(val expense: Expense) : ParseResult
    data class Bad(val line: Int, val reason: String) : ParseResult
}

/* Parse one CSV line; line numbers are 1-based. */
fun parse(lineNo: Int, line: String): ParseResult {
    val parts = line.split(',').map { it.trim() }
    if (parts.size != 3) return ParseResult.Bad(lineNo, "expected 3 fields")
    val category = runCatching { Category.valueOf(parts[1].uppercase()) }.getOrDefault(Category.OTHER)
    val cents = parts[2].toLongOrNull() ?: return ParseResult.Bad(lineNo, "bad amount '${parts[2]}'")
    return ParseResult.Ok(Expense(parts[0], category, cents))
}

val input = args.firstOrNull()?.let { File(it).readLines() }
    ?: listOf("date,category,cents", "2026-01-03,food,1250", "2026-01-04,travel,8900", "oops")
val results = input.drop(1).mapIndexed { i, line -> parse(i + 2, line) }
val totals = results.filterIsInstance<ParseResult.Ok>()
    .groupBy { it.expense.category }
    .mapValues { (_, v) -> v.sumOf { it.expense.cents } }

for ((category, cents) in totals.entries.sortedByDescending { it.value }) {
    println("%-8s %10.2f".format(category, cents / 100.0))
}
for (r in results) when (r) {
    is ParseResult.Bad -> System.err.println("line ${r.line}: ${r.reason}")
    is ParseResult.Ok -> Unit
}
val average = if (totals.isEmpty()) 0 else (totals.values.sum() / totals.size.toDouble()).roundToInt()
println("average per category: $average cents\t(${totals.size} categories, ok=${average >= 0})")
