//! Top-level doc comment for the module — ünïcödé
const std = @import("std");
const builtin = @import("builtin");
const mem = std.mem;
const Allocator = std.mem.Allocator;
const ArrayList = std.ArrayList;

/// A point in 2D space.
pub const Point = struct {
    x: f32 = 0.0,
    y: f32 = 0.0,

    pub fn add(self: Point, other: Point) Point {
        return .{ .x = self.x + other.x, .y = self.y + other.y };
    }

    pub inline fn lengthSquared(self: Point) f32 {
        return self.x * self.x + self.y * self.y;
    }
};

pub const Color = enum(u8) {
    red = 0xff,
    green = 0o17,
    blue = 0b1010_1010,
    _,
};

const Value = union(enum) {
    int: i64,
    float: f64,
    text: []const u8,
    none: void,
};

extern "c" fn printf(format: [*:0]const u8, ...) c_int;
export fn zig_add(a: i32, b: i32) i32 {
    return a +% b;
}

const Packed = packed struct {
	flag: bool,
	mode: u3,
	rest: u4,
};

fn parse(allocator: Allocator, input: []const u8) !ArrayList(u32) {
    var list = ArrayList(u32).init(allocator);
    errdefer list.deinit();
    var it = mem.tokenizeScalar(u8, input, ',');
    while (it.next()) |tok| {
        const n = std.fmt.parseInt(u32, tok, 10) catch |err| switch (err) {
            error.Overflow => return error.TooBig,
            else => continue,
        };
        try list.append(n);
    }
    return list;
}

test "parse numbers" {
    const allocator = std.testing.allocator;
    var list = try parse(allocator, "1,2,3,42");
    defer list.deinit();
    try std.testing.expectEqual(@as(usize, 4), list.items.len);
    try std.testing.expect(list.items[3] == 42);
}

fn strings() void {
    const a = "plain string";
    const b = "escaped \"quote\" and \\ backslash\n\t";
    const c = 'x';
    const d = '\n';
    const e = "unicode: 日本語 🎉 \u{1F600}";
    const multi =
        \\first line of a multiline string
        \\second line with "quotes" inside
        \\  // not a comment
    ;
    const continued = "this string ends with a backslash \
still in the string" ++ " and then some";
    _ = .{ a, b, c, d, e, multi, continued };
}

fn numbers() void {
    const hex = 0xDEAD_BEEF;
    const oct = 0o755;
    const bin = 0b1100;
    const flt = 3.14159;
    const exp = 1.5e10;
    const hexflt = 0x1.8p3;
    const big = 1_000_000;
    const neg = -42;
    _ = .{ hex, oct, bin, flt, exp, hexflt, big, neg };
}

fn operators(x: i32, y: i32) bool {
    var z = x;
    z += y;
    z -= 1;
    z *= 2;
    z <<= 1;
    z >>= 1;
    z |= 0x1;
    z &= 0xF;
    z ^= 3;
    z /= 2;
    const maybe: ?i32 = null;
    const v = maybe orelse 0;
    const ptr = &z;
    ptr.* = v;
    return x == y and x != z or !(x <= y) and x >= y;
}

comptime {
    if (builtin.os.tag == .windows) {
        @compileError("unsupported");
    } else if (builtin.mode == .Debug) {
        // debug build
    }
}

fn generic(comptime T: type, items: []const T) T {
    var sum: T = 0;
    for (items, 0..) |item, i| {
        if (i % 2 == 0) continue;
        sum += item;
    }
    inline for (.{ 1, 2, 3 }) |k| {
        _ = k;
    }
    return sum;
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const stdout = std.io.getStdOut().writer();
    try stdout.print("Hello, {s}! Ⅻ {d}\n", .{ "world", 12 });
    const r = sw: {
        break :sw 1;
    };
    while (true) : (x += 1) {
        break;
    }
    usingnamespace @import("other.zig");
    const noalias_ptr: *noalias u8 = undefined;
    const ünicode_ident = 1;
    _ = r;
    _ = noalias_ptr;
    asm volatile ("nop");
    unreachable;
}

// unterminated string at end of line
const broken = "never closed
const after = 1;
const trailing = "ends with backslash \