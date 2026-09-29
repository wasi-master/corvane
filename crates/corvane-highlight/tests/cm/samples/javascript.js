#!/usr/bin/env node
'use strict';
// Line comment with non-ASCII: café, naïve, 日本語
/* Block comment
   spanning several lines
   with a * star and / slash */
/** JSDoc
 * @param {string} name the name
 * @returns {Promise<void>}
 */

import fs, { readFile as rf, writeFile } from 'fs';
import * as path from "path";
import defaultThing from './thing.js';
import './side-effect.css';
export { rf, writeFile as wf };
export * from './re-export.js';
export default function main() {}
export const answer = 42, other = answer + 1;

const numbers = [0, 1, 42, 3.14, .5, 1., 1e10, 2.5E-3, 1_000_000, 0x1F, 0XAB, 0o17, 0b1010, 123n, 0xffn, 5.e3];
let str1 = 'single \'quoted\' string', str2 = "double \"quoted\" string";
var str3 = "unterminated string
var str4 = 'continued \
on the next line';
const tpl = `plain template`;
const tpl2 = `hello ${name}, you are ${age + 1} years old`;
const tpl3 = `outer ${ `inner ${deep.value} text` } end`;
const multi = `first line
  second line ${ value }
	third line with tab`;
const tagged = html`<div class="${cls}">${content}</div>`;
const nested = `a ${ { key: 'b' }.key } c`;
const escaped = `back\`tick and \${not interpolated}`;

const re1 = /ab+c/gi;
const re2 = /[/\]]+/;
const re3 = /\d{3}-\d{4}/u.test(phone);
const re4 = x / y / z;
const re5 = (a) / 2;
const re6 = arr.map(v => v / 2);
if (/^#/.test(line)) { skip(); }
const re7 = /abc/gg;
const re8 = /unterminated regex
const re9 = str.replace(/\s+/g, ' ');
x /= 3;

function add(a, b = 2, ...rest) {
  return a + b + rest.length;
}

function* generator() {
  yield 1;
  yield* other();
}

async function fetchData(url, { method = 'GET', headers } = {}) {
  try {
    const response = await fetch(url, { method, headers });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return await response.json();
  } catch (err) {
    console.error(err);
  } finally {
    cleanup();
  }
}

const arrow = (x, y) => x * y;
const arrow2 = x => x ** 2;
const arrow3 = async (a, { b, c: [d, e] }) => {
  const sum = a + b + d + e;
  return sum;
};
const arrow4 = () => ({ ok: true });
const asyncArrow = async x => await x;

const { a, b: renamed, c = 3, ...others } = obj;
const [first, , third = 'default', ...tail] = list;

class Animal extends Base {
  #secret = 1;
  static count = 0;
  name = 'animal';
  constructor(name) {
    super(name);
    this.name = name;
    Animal.count++;
  }
  get displayName() { return this.name.toUpperCase(); }
  set displayName(value) { this.name = value; }
  static create(...args) { return new Animal(...args); }
  async load() { await this.#fetch(); }
  *[Symbol.iterator]() { yield this.name; }
  #fetch() { return this.#secret; }
}

const Anon = class extends Animal {};
const instance = new Animal('cat');
const target = new.target;

label: for (let i = 0; i < 10; i++) {
  for (const key in object) {
    if (key === 'skip') continue label;
  }
  for (const item of items) {
    break label;
  }
}
for await (const chunk of stream) process(chunk);

while (running) { tick(); }
do { count--; } while (count > 0);

switch (kind) {
  case 'a':
  case "b":
    handle();
    break;
  default:
    fallback();
}

if (a) b(); else if (c) d(); else { e(); }

const obj2 = {
  key: 'value',
  'quoted-key': 1,
  42: 'number key',
  [computed]: true,
  method() { return 1; },
  get prop() { return this._p; },
  set prop(v) { this._p = v; },
  async asyncMethod() {},
  *gen() {},
  ...spread,
  shorthand,
  nested: { deep: [1, 2, { deeper: null }] },
};

const ops = a + b - c * d / e % f ** g;
const cmp = a == b && a != c || a === d && a !== e;
const bits = a & b | c ^ ~d << 1 >> 2 >>> 3;
const assign = (a += 1, b -= 2, c *= 3, d %= 4, e **= 5, f <<= 1, g >>= 1, h >>>= 1);
const logic = a &&= b, c ||= d, e ??= f;
const opt = obj?.prop?.[key]?.(arg) ?? fallback;
const tern = cond ? yes : no;
const types = typeof x === 'undefined' || x instanceof Foo || 'k' in obj;
delete obj.key; void 0;
const atoms = [true, false, null, undefined, NaN, Infinity, this];
i++; j--; ++k; --l;
x = -y + +z - !w;

debugger;
with (obj) { prop; }
<!-- html comment opener
--> html comment closer at line start
a --> b;
#!not a shebang here
#priv
back\slash;
const ünïcödé = 'π';
const $dollar = _under;

	// tab-indented comment
	const tabbed = true;

/* unterminated block comment
continues here
