//// A tiny shopping cart module.

import gleam/int
import gleam/io
import gleam/list
import gleam/string

/// A line in the cart.
pub type Item {
  Item(name: String, price_cents: Int, quantity: Int)
}

pub type Discount {
  Percent(Int)
  Flat(cents: Int)
  NoDiscount
}

const tax_rate = 0.25

// Total before discount, in cents.
pub fn subtotal(items: List(Item)) -> Int {
  items
  |> list.map(fn(item) { item.price_cents * item.quantity })
  |> list.fold(0, int.add)
}

pub fn apply(total: Int, discount: Discount) -> Int {
  case discount {
    Percent(p) if p > 0 && p <= 100 -> total - total * p / 100
    Percent(_) -> total
    Flat(cents: c) -> int.max(0, total - c)
    NoDiscount -> total
  }
}

pub fn main() {
  let cart = [Item("apple", 50, 4), Item(name: "bread", price_cents: 320, quantity: 1)]
  let total = cart |> subtotal |> apply(Percent(10))
  let assert True = total >= 0
  let tax = int.to_float(total) *. tax_rate
  io.println("Total: " <> int.to_string(total) <> "\tTax: " <> string.inspect(tax))
}
