package game.core;

import haxe.ds.StringMap;
import haxe.Json;
import sys.io.File;
import openfl.display.*;
using StringTools;
using Lambda;

/**
 * Main.hx — entry point for the tile game.
 * Multi-line doc comment with ünïcode: 日本語 ✓
 */
typedef Point = { x:Float, y:Float };
typedef Config = {
    var width:Int;
    var height:Int;
    @:optional var title:String;
}

enum Direction {
    North;
    South;
    East;
    West;
}

enum abstract Color(Int) to Int {
    var Red = 0xFF0000;
    var Green = 0x00ff00;
}

interface IUpdatable {
    function update(dt:Float):Void;
}

@:keep
@:build(macro.Builder.build())
class Entity implements IUpdatable {
    public var name(default, null):String;
    public var pos:Point;
    private static var count:Int = 0;
    static inline var MAX_SPEED = 12.5;
    var hp(get, never):Int;
    dynamic function onDeath():Void {}

    public function new(name:String, ?x:Float = 0, y = 0.0) {
        this.name = name;
        pos = { x: x, y: y };
        count++;
        trace('created $name at ${pos.x}, ${pos.y}');
    }

    function get_hp():Int return 100;

    public function update(dt:Float):Void {
        var speed = MAX_SPEED * dt, damping = 0.95;
        var v:Float = speed - -1;
        pos.x += speed;
        pos.y -= speed * damping;
        if (pos.x > 100 && pos.y < -2.5e3) {
            pos.x = 0;
        } else if (pos.x != pos.x) {
            throw "NaN position";
        } else {
            return;
        }
    }
}

class Main extends Entity {
    static var registry = new StringMap<Entity>();
    static final LIMITS:Array<Int> = [1, 2, 3, -4, 5e2, 1.5, 0xABC, 3.];

    static function main() {
        var cfg:Config = Json.parse(File.getContent("config.json"));
        var re = ~/^([a-z]+)\d*$/gi;
        var escaped = "quote \" and backslash \\ and tab \t";
        var single = 'it\'s ${cfg.width * 2} px';
        var multi = "first line
second line";
        var unterminated = 'still going
        and done';
        var dir:Direction = North;

        switch (dir) {
            case North | South:
                trace("vertical");
            case East:
                trace("east");
            default:
                trace("west");
        }

        for (i in 0...10) {
            if (i % 2 == 0) continue;
            registry.set('e$i', new Entity('e$i', i, i * 2));
        }

        for (e in registry) e.update(1 / 60);

        var total = 0;
        while (total < 100) total += 7;
        do total--; while (total > 50);

        try {
            var n = Std.parseInt("42");
            var f = cast(n, Float);
            var u = untyped __js__("1");
        } catch (e:haxe.Exception) {
            trace(e.message);
        } catch (e:Dynamic) {
            trace('unknown: $e');
        }

        var fn = function(a:Int, b:Int):Int { return a + b; };
        var arrow = (a, b) -> a * b;
        var obj = { name: "box", size: 3, nested: { deep: true } };
        var flags = true || false && !null;
        var bits = (1 << 4) | (0xF0 >>> 2) & ~3 ^ 5;
        var ternary = flags ? "yes" : "no";
        var ok = total >= 10 && total <= 90 || total == 0;
        var list = [for (x in 0...5) x * x];
        var map = ["a" => 1, "b" => 2];
        obj.name = obj.name.toUpperCase().trim();
        callback(fn, 1);
        Main.registry.get("x").pos.x = 1;
    }
}

#if (js && !debug)
@:native("window") extern class Window {}
#elseif cpp
#else
#end

private abstract Meters(Float) from Float to Float {
    public inline function new(v:Float) this = v;
    @:op(A + B) static function add(a:Meters, b:Meters):Meters;
}

	// tab-indented comment
var émoji = "😀 🚀";
var odd = $weird + `backtick + @ + \ ;
var r2 = 1 ~/x/;
/* unterminated block comment at end of file
   continues here