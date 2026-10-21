import type { ExtensionAttr, ExtensionDtype, ExtensionScalar, ExtensionTree } from './contract.js';
import { joinPath, validation } from './errors.js';
import type { ExtraAttrs } from './model.js';

export type RawValue = string | number;
export type FieldDtype = 'utf8' | 'u32' | 'i32' | 'f64';

/** One on-disk attribute of a group. `strict` = missing is an error; otherwise `default` applies. */
export interface FieldSpec {
  readonly name: string;
  readonly dtype: FieldDtype;
  readonly strict: boolean;
  readonly default?: RawValue;
  readonly enum?: readonly string[];
  /** Enum match ignores case on read; the canonical spelling is returned. */
  readonly caseInsensitive?: boolean;
  /** u32 that must be 0 or 1. */
  readonly flag?: boolean;
  readonly nonzero?: boolean;
  readonly startsWith?: string;
}

const U32_MAX = 4294967295;
const I32_MIN = -2147483648;
const I32_MAX = 2147483647;

export function spec(list: readonly FieldSpec[], name: string): FieldSpec {
  const found = list.find((f) => f.name === name);
  if (!found) throw new Error(`internal: no field spec "${name}"`);
  return found;
}

/** Validates `v` against `f` and returns its canonical form. */
export function checkValue(f: FieldSpec, v: unknown, path: string): RawValue {
  const here = joinPath(path, `@${f.name}`);
  if (f.dtype === 'utf8') {
    if (typeof v !== 'string') throw validation(here, `expected a string, got ${typeof v}`);
    if (v.includes('\0')) throw validation(here, 'must not contain NUL');
    let out = v;
    if (f.enum) {
      const match = f.caseInsensitive
        ? f.enum.find((e) => e.toLowerCase() === v.toLowerCase())
        : f.enum.find((e) => e === v);
      if (match === undefined) {
        throw validation(here, `invalid value "${v}"; expected one of ${f.enum.map((e) => `"${e}"`).join(', ')}`);
      }
      out = match;
    }
    if (f.startsWith !== undefined && !(out.startsWith(f.startsWith) && out.length > f.startsWith.length)) {
      throw validation(here, `must start with ${f.startsWith} followed by a name`);
    }
    return out;
  }
  if (typeof v !== 'number' || Number.isNaN(v)) throw validation(here, `expected a number, got ${String(v)}`);
  if (f.dtype === 'f64') {
    if (!Number.isFinite(v)) throw validation(here, 'must be finite');
    return v;
  }
  if (!Number.isInteger(v)) throw validation(here, `expected an integer, got ${v}`);
  const [min, max] = f.dtype === 'u32' ? [0, U32_MAX] : [I32_MIN, I32_MAX];
  if (v < min || v > max) throw validation(here, `${v} is out of range for ${f.dtype}`);
  if (f.flag && v !== 0 && v !== 1) throw validation(here, `flag must be 0 or 1, got ${v}`);
  if (f.nonzero && v === 0) throw validation(here, 'must not be 0');
  return v;
}

export function cloneAttr(attr: ExtensionAttr): ExtensionAttr {
  return { dtype: attr.dtype, value: Array.isArray(attr.value) ? [...attr.value] : attr.value };
}

const DTYPES: readonly string[] = ['utf8', 'i8', 'u8', 'i16', 'u16', 'i32', 'u32', 'i64', 'u64', 'f32', 'f64'];
const INT_RANGES: Record<string, [bigint, bigint]> = {
  i8: [-(2n ** 7n), 2n ** 7n - 1n],
  u8: [0n, 2n ** 8n - 1n],
  i16: [-(2n ** 15n), 2n ** 15n - 1n],
  u16: [0n, 2n ** 16n - 1n],
  i32: [-(2n ** 31n), 2n ** 31n - 1n],
  u32: [0n, 2n ** 32n - 1n],
  i64: [-(2n ** 63n), 2n ** 63n - 1n],
  u64: [0n, 2n ** 64n - 1n],
};

function checkScalar(dtype: ExtensionDtype, v: ExtensionScalar, path: string): void {
  if (dtype === 'utf8') {
    if (typeof v !== 'string') throw validation(path, `utf8 value must be a string, got ${typeof v}`);
    if (v.includes('\0')) throw validation(path, 'utf8 value must not contain NUL');
    return;
  }
  const wantBigint = dtype === 'i64' || dtype === 'u64';
  if (typeof v !== (wantBigint ? 'bigint' : 'number')) {
    throw validation(path, `${dtype} value must be a ${wantBigint ? 'bigint' : 'number'}, got ${typeof v}`);
  }
  if (dtype === 'f32' || dtype === 'f64') return;
  if (typeof v === 'number' && !Number.isInteger(v)) throw validation(path, `${dtype} value must be an integer, got ${v}`);
  const [min, max] = INT_RANGES[dtype];
  const big = BigInt(v as number | bigint);
  if (big < min || big > max) throw validation(path, `${dtype} value ${big} is out of range`);
}

/** Validates an attribute the codec carries opaquely (name, dtype and value shape). */
export function checkExtraAttr(name: string, attr: ExtensionAttr, path: string): void {
  const here = joinPath(path, `@${name}`);
  if (name.length === 0 || name.includes('/') || name.includes('\0')) throw validation(here, 'invalid attribute name');
  if (!DTYPES.includes(attr.dtype)) throw validation(here, `unknown dtype "${String(attr.dtype)}"`);
  if (Array.isArray(attr.value)) attr.value.forEach((v, i) => checkScalar(attr.dtype, v, `${here}[${i}]`));
  else checkScalar(attr.dtype, attr.value, here);
}

export function sortedRecord<T>(entries: Record<string, T>): Record<string, T> {
  const out: Record<string, T> = {};
  for (const k of Object.keys(entries).sort()) out[k] = entries[k];
  return out;
}

export function assertLeaf(tree: ExtensionTree, path: string): void {
  const dataset = Object.keys(tree.datasets)[0];
  if (dataset !== undefined) throw validation(joinPath(path, dataset), 'unexpected dataset');
  const child = Object.keys(tree.children)[0];
  if (child !== undefined) throw validation(joinPath(path, child), 'unexpected child group');
}

export function requireChild(tree: ExtensionTree, name: string): ExtensionTree {
  const child = tree.children[name];
  if (!child) throw validation(name, 'missing group');
  return child;
}

export interface ReadFields {
  values: Record<string, RawValue>;
  extra: ExtraAttrs;
}

/** Reads every spec'd attribute (exact dtype, validated) and collects the rest into `extra`. */
export function readFields(
  attrs: Record<string, ExtensionAttr>,
  specs: readonly FieldSpec[],
  path: string,
  ignore: readonly string[] = [],
): ReadFields {
  const values: Record<string, RawValue> = {};
  for (const f of specs) {
    const a = attrs[f.name];
    if (a === undefined) {
      if (f.strict) throw validation(joinPath(path, `@${f.name}`), 'missing required attribute');
      values[f.name] = f.default as RawValue;
      continue;
    }
    const here = joinPath(path, `@${f.name}`);
    if (a.dtype !== f.dtype) throw validation(here, `expected dtype ${f.dtype}, got ${a.dtype}`);
    if (Array.isArray(a.value)) throw validation(here, 'expected a scalar, got an array');
    values[f.name] = checkValue(f, a.value, path);
  }
  const known = new Set<string>([...specs.map((f) => f.name), ...ignore]);
  const extra: ExtraAttrs = {};
  for (const name of Object.keys(attrs).sort()) {
    if (!known.has(name)) extra[name] = cloneAttr(attrs[name]);
  }
  return { values, extra };
}

/** Writes every spec'd attribute with its exact dtype, plus `extra`; output is sorted by name. */
export function writeFields(
  values: Record<string, RawValue>,
  specs: readonly FieldSpec[],
  extra: ExtraAttrs,
  path: string,
  reserved: readonly string[] = [],
): Record<string, ExtensionAttr> {
  const out: Record<string, ExtensionAttr> = {};
  for (const f of specs) out[f.name] = { dtype: f.dtype, value: checkValue(f, values[f.name], path) };
  if (extra === null || typeof extra !== 'object') throw validation(joinPath(path, '@extra'), 'extra must be an object');
  for (const [name, attr] of Object.entries(extra)) {
    if (name in out || reserved.includes(name)) {
      throw validation(joinPath(path, `@${name}`), 'extra attribute collides with a known attribute');
    }
    checkExtraAttr(name, attr, path);
    out[name] = cloneAttr(attr);
  }
  return sortedRecord(out);
}

/** Typed accessors over {@link ReadFields.values}. */
export class Values {
  constructor(private readonly v: Record<string, RawValue>) {}
  str(name: string): string {
    return this.v[name] as string;
  }
  num(name: string): number {
    return this.v[name] as number;
  }
  flag(name: string): boolean {
    return this.v[name] === 1;
  }
}

export function isLabel(label: unknown): label is string {
  return typeof label === 'string' && label.length > 0 && !label.includes('/') && !label.includes('\0');
}
