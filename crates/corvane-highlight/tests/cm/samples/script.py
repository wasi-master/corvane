#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Inventory sync — a realistic sample for the highlighter.

Covers f-strings, bytes, raw strings, decorators, match, async and more.
"""
from __future__ import annotations

import asyncio, os, re as regex
from dataclasses import dataclass, field
from typing import Any, Callable, Optional, TypeVar

T = TypeVar("T")
__all__ = ['Item', "sync"]
PI = 3.14159; E = 2.718e0; TAU = .5e-3
HEX, BIN, OCT = 0xFF_FF, 0b1010_0101, 0o755
BIG = 1_000_000; ZERO = 0; IMAG = 3j + 2.5J; LONGISH = 10L
weird = 1.2.3 + 00 + 0x + 1e5 + 1E+5 + 7e-2 + 0X1f + 0B11 + 0O7
ellipsis = ...


@dataclass(frozen=True)
class Item:
    """A stock-keeping unit."""
    name: str
    price: float = 0.0
    tags: list[str] = field(default_factory=list)

    @property
    def label(self) -> str:
        return f"{self.name!r:>20} costs {self.price:.2f} €"

    @staticmethod
    def parse(line: str) -> "Item":
        name, _, price = line.partition("=")
        return Item(name.strip(), float(price))

    @classmethod
    def empty(cls): return cls("")


def build(items: list[Item], *, key: Callable[[Item], Any] = len, **kw) -> dict[str, Item]:
    result = {}
    for i, it in enumerate(items):
        if (n := len(it.name)) > 10 and not it.tags or it is None:
            continue
        elif i % 2 == 0:
            result[it.name] = it
        else:
            pass
    return result


@app.route("/items/<int:id>", methods=["GET", 'POST'])
@functools.lru_cache(maxsize=None)
def handler(request, id: int = 0) -> Optional[dict]:
    x = lambda a, b=2: a ** b // 3
    y = [v for v in range(10) if v & 1 | 2 ^ 4]
    z = {k: v for k, v in zip("abc", b"xyz")}
    x @= y
    x <<= 2; x >>= 1; x != y; x <> y
    x -= 1; x += ~1; x *= -1; x /= 2; x //= 3; x %= 4; x **= 2
    return None


async def fetch(session, url: str) -> bytes:
    async with session.get(url) as resp:
        await asyncio.sleep(0.1)
        async for chunk in resp.content:
            yield chunk
    raise RuntimeError("unreachable")


def strings():
    a = 'single' + "double" + '''triple
single''' + """triple
double"""
    b = b'bytes' + B"BYTES" + br'raw\bytes' + Rb"raw" + rB'x' + bR"y"
    c = r'C:\path\no\escape' + R"raw" + u'unicode' + U"uni"
    d = f'{a}' + F"{b!s}" + fr'{c}\n' + Rf"{d}" + rF'{e}' + FR"{f}"
    e = f"{{literal braces}} and {value:{width}.{precision}}"
    f = f"nested {'quotes' if cond else "other"} and {d['key']}"
    g = f"dict {  {'a': 1}['a']  } ok"
    h = f"unbalanced } brace"
    i = f'''multi {x
        + y} line''' + f"""
    {z!r}
    """
    j = "escaped \" quote" + 'it\'s' + "tab\there"
    k = "line continues \
here"
    l = 'unterminated
    m = f"also unterminated {x
    n = f"{x # not a comment}" + f"{1:#x}"
    return a, b, c, d, e, f, g, h, i, j, k


class Shape(object):
    def __init__(self, sides=4):
        self.sides = sides
        super().__init__()

    def area(self): ...

    def __repr__(self):
        return "<%s sides=%d>" % (type(self).__name__, self.sides)


def dispatch(command):
    match command.split():
        case [action]:
            return action
        case ["go", direction] | ["move", direction]:
            return direction
        case {"x": x, "y": y, **rest}:
            return x, y
        case Point(x=0, y=0):
            print("Origin")
        case _:
            raise ValueError(f"unknown: {command}")


total = (1 +
         2 +
    3)
values = [
    1, 2,
    3,
]
call(arg,
     kw=value)
if total > 3 and \
        values:
    print("continued")
x = 1 \
    + 2

try:
    import missing
except (ImportError, ModuleNotFoundError) as exc:
    print(exc, file=sys.stderr)
finally:
    del x
    global y
    nonlocal_var = None
    assert True, "never"

while False: break
with open("f") as fh, open("g") as gh: data = fh.read()
    oddly_indented = 1
  dedented_wrong = 2
closing = )]
opening = ([{
mismatch = (]
}
	tabbed = "tab indent"
名前 = "unicode identifier"; café = naïve + résumé
print(名前, café, sep="→")
value = obj.if_ + obj.class_ + obj.def + obj.None + self.x + cls.y
def  spaced  (a) : pass
class Foo(Base, metaclass=Meta): pass
$ ? ! `backtick`
@
@ decorator_with_space
    @nested.deco(1)
    def inner(): return 0
print(eval("1+1"), exec, breakpoint(), __import__("os"), NotImplemented, Ellipsis)
if __name__ == "__main__":
    asyncio.run(main())

s = f"{f'{x!r}'}" + f"{'{'}" + f"{x:{'>'}{10}}" + f"a {b} \
continued {c} line"
t = f"""{
    a +
    b
}""" + rb"""raw
bytes""" + f"{lambda: 1}" + f"{x:=5}"
def gen():
	yield from range(3)
	return
x = [i async for i in aiter() if await i]
print(x) ; print (y)
