import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { pack, peekSchemaVersion, unpack } from '../src/index.js';
import { FIXTURES, fixtureModel, fixtureTree } from './helpers/fixtures.js';
import { cloneTree, treeToJson } from './helpers/treeJson.js';

const V1_2 = readdirSync(join(FIXTURES, 'v1_2'));
// Trees written in canonical form: pack(model) must reproduce them exactly.
const CANONICAL = ['minimal', 'rich'];

describe('v1_2 fixtures', () => {
  it('has at least 3 cases', () => {
    expect(V1_2.length).toBeGreaterThanOrEqual(3);
  });

  for (const name of V1_2) {
    const rel = `v1_2/${name}`;

    it(`${name}: unpack(tree) equals model`, () => {
      expect(unpack(fixtureTree(rel))).toEqual({ ok: true, value: fixtureModel(rel) });
    });

    it(`${name}: pack(model) re-unpacks to the model`, () => {
      const packed = pack(fixtureModel(rel));
      if (!packed.ok) throw new Error(packed.error.message);
      expect(peekSchemaVersion(packed.value)).toBe('1.2');
      expect(unpack(packed.value)).toEqual({ ok: true, value: fixtureModel(rel) });
    });
  }

  for (const name of CANONICAL) {
    it(`${name}: pack(model) equals tree`, () => {
      const packed = pack(fixtureModel(`v1_2/${name}`));
      if (!packed.ok) throw new Error(packed.error.message);
      expect(treeToJson(packed.value)).toEqual(treeToJson(cloneTree(fixtureTree(`v1_2/${name}`))));
    });
  }

  it('lenient_read normalises casing, sentinels, ordering and unknown minor', () => {
    const tree = fixtureTree('v1_2/lenient_read');
    expect(peekSchemaVersion(tree)).toBe('1.7');
    const packed = pack(fixtureModel('v1_2/lenient_read'));
    if (!packed.ok) throw new Error(packed.error.message);
    expect(packed.value.children.Client.attrs.Machine_Profile.value).toBe('Aconity');
    expect(packed.value.children.Pipe.attrs.Min_Integrity_Level.value).toBe('High');
    expect(packed.value.children.Pipe.attrs.Inbound_Rate_Limit.value).toBe(0);
    expect(packed.value.attrs.Opcua_Schema_Version.value).toBe('1.2');
  });
});
