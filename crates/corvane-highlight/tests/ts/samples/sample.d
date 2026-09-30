/// A small word-frequency counter.
module sample;

import std.stdio : writeln, writefln;
import std.algorithm : sort, splitter;
import std.uni : toLower;

enum Mode { strict, relaxed }

immutable size_t minLength = 3;

struct Entry {
    string word;
    size_t count;
    string toString() const @safe {
        import std.format : format;
        return format("%s=%d", word, count);
    }
}

/* Counts words, optionally skipping short ones. */
class Counter {
    private size_t[string] table;
    Mode mode = Mode.relaxed;
    void add(string text) {
        foreach (w; text.splitter(' ')) {
            if (w.length < minLength && mode == Mode.strict) continue;
            table[w.toLower] += 1;
        }
    }

    Entry[] entries() {
        Entry[] result;
        foreach (k, v; table) result ~= Entry(k, v);
        result.sort!((a, b) => a.count > b.count);
        return result;
    }
}

void main() {
    auto c = new Counter;
    c.add("the quick brown fox jumps over the lazy dog, the end");
    foreach (e; c.entries()[0 .. 3]) writeln(e); // three most common
    writefln("total: %d, hex: %x, ok: %s\n", c.entries().length, 0xFF, true);
}
