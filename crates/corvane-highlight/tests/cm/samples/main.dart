// Dart sample — Ünïcödé: señor, Ωmega, 中文
/* block /* nested */ still comment */
/// Doc comment for the library.
library corvane.sample;

import 'dart:async';
import 'dart:math' as math show Random, pi;
import 'package:flutter/material.dart' hide Colors;
export 'src/model.dart';
part 'src/part.dart';

@immutable
class Point {
  final double x, y;
  const Point(this.x, this.y);
  Point.origin() : this(0, 0);
  static const zero = Point(0, 0);
  double get length => math.sqrt(x * x + y * y);
  Point operator +(Point other) => Point(x + other.x, y + other.y);
  @override
  String toString() => 'Point($x, $y) with ${x + y} and ${'nested $x'}';
}

abstract class Shape {
  double area();
}

mixin Logger on Object {
  void log(String message) => print('[${runtimeType}] $message');
}

sealed class Result<T> {}
base class Ok<T> extends Result<T> { final T value; Ok(this.value); }
enum Color { red, green, blue }
extension on String { String get shout => toUpperCase(); }
typedef Handler = void Function(String event);

Future<void> main(List<String> args) async {
  var count = 0;
  final int hex = 0xFF, big = 1000000;
  const double ratio = 2.5e-3, half = .5;
  num n = 42;
  dynamic d = null;
  bool flag = true && !false;
  String s1 = 'single $count';
  String s2 = "double ${count + 1} done";
  String raw = r'raw $notInterpolated \n';
  String rawDouble = r"raw ${also} not";
  String escaped = 'it\'s \$escaped \\';
  String empty = '', emptyDouble = "";
  String multi = '''
    Triple single with $count and ${count * 2}
    and 'quotes' inside
    ''';
  String multiDouble = """one line triple""";
  final list = <int>[1, 2, 3].map((e) => e * 2).where((e) => e > 2).toList();
  final map = {'a': 1, 'b': 2};
  final set = <String>{'x', 'y'};
  late final String lazyValue;
  for (final item in list) {
    if (item % 2 == 0) continue;
    else if (item > 4) break;
  }
  switch (count) {
    case 0:
      print('zero');
    default:
      print('other');
  }
  var shape = switch (count) { 0 => 'none', _ when count > 1 => 'many', _ => 'one' };
  try {
    await Future.delayed(const Duration(milliseconds: 10));
    throw StateError('boom');
  } on StateError catch (e, st) {
    print('$e $st');
    rethrow;
  } finally {
    count++;
  }
  assert(count >= 0, 'never negative');
  Stream<int> gen() async* { yield 1; yield* Stream.value(2); }
  final _PrivateClass p = _PrivateClass();
  final $dollar = 1;
  String broken = 'unterminated
  String open = """
never closed with $interp
