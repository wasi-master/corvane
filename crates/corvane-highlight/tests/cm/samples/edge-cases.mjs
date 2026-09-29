// Edge cases for the JavaScript parser's quirks
(a, b) => a + b
foo(x => x)
next line at column zero
bar
const fn = function named(p) { return p; };
const nestedArrow = (a) => (b) => (c) => a + b + c;
const withDefault = (a = () => 1, b = [1, 2]) => a() + b[0];
const strArrow = (s = ")") => s;
const commentArrow = (a /* c */) => a;
const arrowObj = ({ x = 1, y: { z } }) => x + z;
promise.then(result => { console.log(result); return result; }).catch(() => {});
setTimeout(() => void run(), 100);

function scopes(param) {
  var hoisted = 1;
  let blockLet = 2;
  {
    var inner = 3;
    let innerLet = 4;
    param + hoisted + blockLet + inner + innerLet;
  }
  for (var i = 0; i < 3; i++) { i; }
  return function () { return param + hoisted + arguments.length + this; };
}
outsideScope + param;

const regexAfterQuasi = `${ /re}/.source }`;
const regexInArray = [/a/, /b/g];
const regexAfterReturn = () => { return /x/; };
const division = total / count / 2;
const divAfterParen = (a + b) / (c - d);
const divAfterBracket = arr[0] / arr[1];
const regexFlags = [/a/gimsuy, /b/dg, /c/gg, /d/ig2];
const charClass = /[\/[\]]/;
const escapedSlash = /a\/b/;
typeof /re/;
x = y
/re/g.test(s);

const quasiInQuasi = `level1 ${`level2 ${`level3 ${deep}`}`}`;
const quasiObject = `${{ a: 1 }.a}`;
const quasiFunction = `${(() => 'iife')()}`;
const quasiUnclosed = `open ${ value
  continues } still template
closed`;
const afterTemplate = 1;

obj.if = obj.class + obj.new + obj.return;
obj.async = async;
async
function afterAsync() {}
const asyncMethod = { async *gen() {}, async get() {} };
const kw = { if: 1, class: 2, default: 3, new: 4 };

class Fields {
  static #privateStatic = 1;
  static { init(); }
  static async *method() {}
  get = 1;
  set;
  static;
  'string key'() {}
  42() {}
  [computed] = 2;
}

export const { destructured, other: renamed } = source;
export function* genExport() {}
export async function asyncExport() {}
export default class DefaultClass {}
import json from './data.json' with { type: 'json' };
import('./dynamic.js').then(m => m.default);
import.meta.url;

new Foo;
new Foo.Bar();
new (getClass())();
const x2 = new.target?.name;

a
++b
c = d
--e

try { risky(); } catch { recover(); }
try { risky(); } catch ({ message }) { log(message); }

throw new TypeError("bad");
if (x) return
else return;

label2:
while (true) break label2;

0.5.toFixed(2);
1..toString();
1.5e+10;
0b;
0x;
08;
.e5;
1_2_3.4_5e6_7;
99999999999999999999n;

"a" + 'b' + `c`;
'\u{1F600}';
"\x41A\n\t\\";
'multi \
line \
continuation';

a?.b?.c;
a ?. b;
x ? .5 : .6;
cond ?.3:.4;

z = a < b > c;
generic<Type>(arg);
jsxLike = <div>;

@decorator
class Decorated {}
