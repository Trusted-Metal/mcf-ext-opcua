import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFileSync } from 'node:fs';
import type { ExtensionTree } from '../../src/contract.js';
import type { OpcuaConfig } from '../../src/model.js';
import { cloneTree, loadTree, modelFromJson } from './treeJson.js';

export const FIXTURES = join(fileURLToPath(new URL('.', import.meta.url)), '../../../fixtures');

export function fixtureTree(rel: string): ExtensionTree {
  return loadTree(join(FIXTURES, rel, 'tree.json'));
}

export function fixtureModel(rel: string): OpcuaConfig {
  return modelFromJson<OpcuaConfig>(JSON.parse(readFileSync(join(FIXTURES, rel, 'model.json'), 'utf8')));
}

/** A fresh copy of the rich v1.2 fixture tree to mutate in error tests. */
export function richTree(): ExtensionTree {
  return cloneTree(fixtureTree('v1_2/rich'));
}

export function richModel(): OpcuaConfig {
  return fixtureModel('v1_2/rich');
}

export function group(tree: ExtensionTree, ...names: string[]): ExtensionTree {
  let g = tree;
  for (const n of names) g = g.children[n];
  return g;
}
