// TypeScript sample
import type { Foo, Bar } from './types';
import { Component, type Props } from 'lib';

export interface User {
  id: number;
  name: string;
  email?: string;
  readonly createdAt: Date;
  tags: string[];
  meta: Record<string, unknown>;
  [key: string]: any;
  greet(message: string): void;
  callback: (err: Error | null, data?: Buffer) => void;
}

interface Admin extends User, Auditable {
  permissions: Array<'read' | 'write'>;
}

export type ID = string | number;
type Mapped<T> = { readonly [K in keyof T]?: T[K] };
type Cond<T> = T extends string ? 'str' : T extends number ? 'num' : never;
type Fn = (a: number, b?: string, ...rest: boolean[]) => Promise<void>;
type Tpl = `prefix-${string}-suffix`;
type Tuple = [string, number, ...boolean[]];
type Inferred<T> = T extends Promise<infer U> ? U : T;
type Keys = keyof typeof config;

enum Color { Red, Green = 'green', Blue = 4 }
const enum Direction { Up = 1, Down }
declare const VERSION: string;
declare module 'foo' {
  export function bar(): void;
}
namespace Utils {
  export const pi = 3.14;
}

abstract class Shape<T extends object = {}> implements Drawable, Serializable {
  private readonly id: number;
  protected name?: string;
  public static count: number = 0;
  #hidden!: boolean;
  abstract area(): number;
  constructor(private x: number, public y: number, readonly z = 0) {
    super();
  }
  get size(): number { return this.x * this.y; }
  method<U>(arg: U, opt?: T): U | undefined {
    return arg as U;
  }
}

@Component({ selector: 'app-root', template: '<div></div>' })
class AppComponent {
  @Input() value: string = '';
  @Output() changed = new EventEmitter<string>();
  constructor(@Inject(TOKEN) private readonly svc: Service) {}
}

function generic<T, K extends keyof T>(obj: T, key: K): T[K] {
  return obj[key];
}
function isString(x: unknown): x is string {
  return typeof x === 'string';
}
function assertDefined<T>(v: T | undefined): asserts v {
  if (v === undefined) throw new Error('undefined!');
}

const typed: Map<string, number[]> = new Map<string, number[]>();
const cast = <HTMLElement>document.body;
const asCast = value as unknown as string;
const nonNull = maybe!.value!;
const arrowTyped = (a: number, b: number): number => a + b;
const arrowGeneric = <T,>(x: T): T => x;
const objTyped = ({ a, b }: { a: number; b: string }): void => {};
let optionalCall = fn?.<string>(arg);
const sat = { a: 1 } satisfies Record<string, number>;

export default class extends Base<Props, State> {}
export abstract class Repo<T extends { id: ID }> {}

let x: number = 1, y: string, z: boolean | null = null;
let fnType: { (a: string): number; prop: string };
let ctor: new (...args: any[]) => object;
let unique: unique symbol;
for (const [k, v] of Object.entries(obj) as [string, number][]) {}

/* multi-line
   type comment */
type LongUnion =
  | 'alpha'
  | 'beta'
  | 'gamma';
