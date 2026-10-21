import { describe, expect, it } from 'vitest';
import {
  CURRENT_SCHEMA_VERSION,
  EXTENSION_NAME,
  REQUIRES_CONTRACT,
  pack,
  peekSchemaVersion,
  suggestedDefaults,
  supportedSchemaVersions,
  unpack,
} from '../src/index.js';
import { richModel, richTree } from './helpers/fixtures.js';

describe('facade', () => {
  it('exports constants', () => {
    expect(EXTENSION_NAME).toBe('TM_OPCUA');
    expect(REQUIRES_CONTRACT).toBe('^0.1');
    expect(CURRENT_SCHEMA_VERSION).toBe('1.2');
    expect(supportedSchemaVersions()).toEqual(['1.2']);
  });

  it('peekSchemaVersion', () => {
    expect(peekSchemaVersion(richTree())).toBe('1.2');
    const t = richTree();
    delete t.attrs.Opcua_Schema_Version;
    expect(peekSchemaVersion(t)).toBe('legacy');
    t.attrs.Opcua_Schema_Version = { dtype: 'utf8', value: '3.1' };
    expect(peekSchemaVersion(t)).toBe('3.1');
  });

  it('any 1.x minor is read by the v1 adapter', () => {
    for (const v of ['1.0', '1.2', '1.9']) {
      const t = richTree();
      t.attrs.Opcua_Schema_Version.value = v;
      expect(unpack(t).ok).toBe(true);
    }
  });

  it('pack targetSchema', () => {
    expect(pack(richModel(), { targetSchema: '1.2' }).ok).toBe(true);

    const legacy = pack(richModel(), { targetSchema: 'legacy' });
    expect(legacy.ok).toBe(false);
    if (!legacy.ok) expect(legacy.error.code).toBe('LossyDowngrade');

    const future = pack(richModel(), { targetSchema: '2.0' });
    expect(future.ok).toBe(false);
    if (!future.ok) expect(future.error.code).toBe('UnsupportedSchemaVersion');
  });

  it('suggestedDefaults differ for Aconity only where documented', () => {
    const base = suggestedDefaults();
    const aconity = suggestedDefaults('Aconity');
    expect(base.client.samplingInterval).toEqual({ kind: 'Ms', ms: 250 });
    expect(aconity.client.samplingInterval).toEqual({ kind: 'Ms', ms: 50 });
    expect(base.client.queueSizeDataChange).toBe(10);
    expect(aconity.client.queueSizeDataChange).toBe(50);
    expect(suggestedDefaults('Eos')).toEqual(base);
  });

  it('suggestedDefaults produce a packable config', () => {
    const d = suggestedDefaults();
    const r = pack({
      client: { ...d.client, serverUrl: 'opc.tcp://h:1', extra: {} },
      pipe: { ...d.pipe, extra: {} },
      triggers: { enabled: false, rules: [], extra: {} },
      extra: {},
    });
    expect(r.ok).toBe(true);
  });
});
