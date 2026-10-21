// Copy of core's tree JSON form (i64/u64 as decimal strings), so tests never import core.
import { readFileSync } from 'node:fs';
import { emptyTree, type ExtensionAttr, type ExtensionDtype, type ExtensionScalar, type ExtensionTree } from '../../src/contract.js';

export type JsonScalar = string | number;
export interface AttrJson {
  dtype: ExtensionDtype;
  value: JsonScalar | JsonScalar[];
}
export interface TreeJson {
  attrs: Record<string, AttrJson>;
  children: Record<string, TreeJson>;
  datasets?: Record<string, unknown>;
}

function scalarToJson(v: ExtensionScalar): JsonScalar {
  if (typeof v === 'bigint') return v.toString();
  if (typeof v === 'number' && !Number.isFinite(v)) return Number.isNaN(v) ? 'NaN' : v > 0 ? 'Infinity' : '-Infinity';
  return v;
}

function scalarFromJson(dtype: ExtensionDtype, v: JsonScalar): ExtensionScalar {
  if (dtype === 'i64' || dtype === 'u64') return BigInt(v as string);
  if (typeof v === 'string' && dtype !== 'utf8') return v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : -Infinity;
  return v;
}

export function attrToJson(a: ExtensionAttr): AttrJson {
  return { dtype: a.dtype, value: Array.isArray(a.value) ? a.value.map(scalarToJson) : scalarToJson(a.value) };
}

export function attrFromJson(a: AttrJson): ExtensionAttr {
  return {
    dtype: a.dtype,
    value: Array.isArray(a.value) ? a.value.map((v) => scalarFromJson(a.dtype, v)) : scalarFromJson(a.dtype, a.value),
  };
}

export function treeToJson(tree: ExtensionTree): TreeJson {
  const children: Record<string, TreeJson> = {};
  for (const [k, c] of Object.entries(tree.children)) children[k] = treeToJson(c);
  const attrs: Record<string, AttrJson> = {};
  for (const [k, a] of Object.entries(tree.attrs)) attrs[k] = attrToJson(a);
  return { attrs, children, datasets: {} };
}

export function treeFromJson(json: TreeJson): ExtensionTree {
  const tree = emptyTree();
  for (const [k, a] of Object.entries(json.attrs)) tree.attrs[k] = attrFromJson(a);
  for (const [k, c] of Object.entries(json.children)) tree.children[k] = treeFromJson(c);
  return tree;
}

export function loadTree(path: string): ExtensionTree {
  return treeFromJson(JSON.parse(readFileSync(path, 'utf8')) as TreeJson);
}

export function cloneTree(tree: ExtensionTree): ExtensionTree {
  return treeFromJson(treeToJson(tree));
}

/** Model JSON: only the `extra` objects hold attributes (with i64/u64 as strings). */
export function modelFromJson<T>(json: unknown): T {
  const walk = (node: unknown, key?: string): unknown => {
    if (Array.isArray(node)) return node.map((n) => walk(n));
    if (node !== null && typeof node === 'object') {
      const out: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(node)) {
        out[k] = k === 'extra' ? Object.fromEntries(Object.entries(v as Record<string, AttrJson>).map(([n, a]) => [n, attrFromJson(a)])) : walk(v, k);
      }
      return out;
    }
    return node;
  };
  return walk(json) as T;
}

export function modelToJson(model: unknown): unknown {
  const walk = (node: unknown): unknown => {
    if (Array.isArray(node)) return node.map(walk);
    if (node !== null && typeof node === 'object') {
      const out: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(node)) {
        out[k] = k === 'extra' ? Object.fromEntries(Object.entries(v as Record<string, ExtensionAttr>).map(([n, a]) => [n, attrToJson(a)])) : walk(v);
      }
      return out;
    }
    return node;
  };
  return walk(model);
}
