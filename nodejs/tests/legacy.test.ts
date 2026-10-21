import { describe, expect, it } from 'vitest';
import {
  emptyTree,
  pack,
  peekSchemaVersion,
  unpack,
  type ExtensionAttr,
  type ExtensionTree,
  type OpcuaConfig,
} from '../src/index.js';
import { fixtureModel, fixtureTree } from './helpers/fixtures.js';

const LEGACY = 'legacy/reference_config_opcua';

const utf8 = (value: string): ExtensionAttr => ({ dtype: 'utf8', value });
const i32 = (value: number): ExtensionAttr => ({ dtype: 'i32', value });
const i64 = (value: bigint): ExtensionAttr => ({ dtype: 'i64', value });
const f64 = (value: number): ExtensionAttr => ({ dtype: 'f64', value });

function legacyTree(opts: {
  client?: Record<string, ExtensionAttr>;
  pipe?: Record<string, ExtensionAttr>;
  triggers?: ExtensionTree | null;
} = {}): ExtensionTree {
  const t = emptyTree();
  t.children.Client = {
    ...emptyTree(),
    attrs: {
      Server_URL: utf8('opc.tcp://h:1'),
      Auth_Mode: utf8('Anonymous'),
      Security_Mode: utf8('None'),
      Security_Policy: utf8('None'),
      Sampling_Interval: i64(250n),
      ...opts.client,
    },
  };
  t.children.Pipe = { ...emptyTree(), attrs: { ...opts.pipe } };
  if (opts.triggers !== null && opts.triggers !== undefined) t.children.Triggers = opts.triggers;
  return t;
}

function triggersTree(groups: Record<string, Record<string, ExtensionAttr>>, attrs: Record<string, ExtensionAttr> = {}): ExtensionTree {
  const t = { ...emptyTree(), attrs };
  for (const [name, a] of Object.entries(groups)) t.children[name] = { ...emptyTree(), attrs: a };
  return t;
}

function ok(tree: ExtensionTree): OpcuaConfig {
  const r = unpack(tree);
  if (!r.ok) throw new Error(`${r.error.code} ${r.error.path}: ${r.error.message}`);
  return r.value;
}

describe('legacy reference fixture', () => {
  it('is detected as legacy', () => {
    expect(peekSchemaVersion(fixtureTree(LEGACY))).toBe('legacy');
  });

  it('unpack(tree) equals model.json', () => {
    expect(unpack(fixtureTree(LEGACY))).toEqual({ ok: true, value: fixtureModel(LEGACY) });
  });

  it('pins the values read from the h5', () => {
    const m = fixtureModel(LEGACY);
    expect(m.client.serverUrl).toBe('opc.tcp://172.17.20.240:62541/TM_OPCUA_DevTemplate_V0.1/TelemetryServer');
    expect(m.client.authMode).toBe('UsernamePassword');
    expect(m.triggers.rules.map((r) => r.label)).toEqual([
      'Chamber Oxygen Level - Start',
      'Chamber Oxygen Level - Stop',
      'Laser Emission Interlock - Start',
      'Laser Emission Interlock - Stop',
    ]);
    expect(m.triggers.rules.map((r) => r.id)).toEqual([1, 2, 3, 4]);
  });

  it('unpack -> pack -> unpack is stable (except warnings)', () => {
    const first = ok(fixtureTree(LEGACY));
    const packed = pack(first);
    if (!packed.ok) throw new Error(packed.error.message);
    const second = ok(packed.value);
    const { warnings: _w1, ...a } = first;
    const { warnings: _w2, ...b } = second;
    expect(b).toEqual(a);
    expect(_w2).toBeUndefined();
  });

  it('packed output validates through the v1 adapter and reports 1.2', () => {
    const packed = pack(ok(fixtureTree(LEGACY)));
    if (!packed.ok) throw new Error(packed.error.message);
    expect(peekSchemaVersion(packed.value)).toBe('1.2');
    expect(unpack(packed.value).ok).toBe(true);
  });
});

describe('legacy conversion', () => {
  it('applies documented defaults to missing optional attributes', () => {
    const c = ok(legacyTree());
    expect(c.client).toEqual({
      serverUrl: 'opc.tcp://h:1',
      authMode: 'Anonymous',
      securityMode: 'None',
      securityPolicy: 'Auto',
      rootNode: { kind: 'AutoDiscover' },
      samplingInterval: { kind: 'Ms', ms: 250 },
      publishInterval: 250,
      bfsMaxDepth: 16,
      keepAliveCount: 240,
      lifetimeCount: 2400,
      queuePolicy: 'DiscardOldest',
      queueSizeDataChange: 10,
      queueSizeEvents: 10,
      reconnectInterval: 1000,
      sessionTimeout: 60000,
      syncLoopIntervalInitial: 1000,
      syncLoopIntervalSettled: 30000,
      extra: {},
    });
    expect(c.pipe).toEqual({
      enabled: false,
      configureClient: false,
      bufferSize: 65536,
      inboundRateLimitHz: 100,
      maxInboundMessageSize: 4096,
      minIntegrityLevel: 'ProtectedProcess',
      pipeName: '\\\\.\\pipe\\opc_ua_client_pipe',
      userAccessLevel: 'CurrentUserOnly',
      extra: {},
    });
    expect(c.triggers).toEqual({ enabled: false, rules: [], extra: {} });
    expect(c.warnings).toEqual([]);
  });

  it('accepts integers as i32, i64 or integral floats', () => {
    const c = ok(
      legacyTree({
        client: { BFS_Max_Depth: i64(8n), Publish_Interval: f64(500), Session_Timeout: i32(1234) },
        pipe: { Pipe_Enabled: f64(1), Buffer_Size: i64(2048n) },
        triggers: triggersTree({}, { Triggers_Enabled: f64(1), Trigger_Stop_Ceiling_Layers: i64(7n) }),
      }),
    );
    expect(c.client.bfsMaxDepth).toBe(8);
    expect(c.client.publishInterval).toBe(500);
    expect(c.client.sessionTimeout).toBe(1234);
    expect(c.pipe.enabled).toBe(true);
    expect(c.pipe.bufferSize).toBe(2048);
    expect(c.triggers.enabled).toBe(true);
    expect(c.triggers.stopCeilingLayers).toBe(7);
  });

  it('maps legacy vocabulary', () => {
    expect(ok(legacyTree({ client: { Machine_Profile: utf8('eos') } })).client.machineProfile).toBe('Eos');
    expect(ok(legacyTree({ client: { Machine_Profile: utf8('ACONITY') } })).client.machineProfile).toBe('Aconity');
    expect(ok(legacyTree({ client: { Machine_Profile: utf8('') } })).client.machineProfile).toBeUndefined();
    expect(ok(legacyTree({ client: { Queue_Policy: utf8('DropOldest') } })).client.queuePolicy).toBe('DiscardOldest');
    expect(ok(legacyTree({ client: { Queue_Policy: utf8('KeepOldest') } })).client.queuePolicy).toBe('KeepOldest');
    expect(ok(legacyTree({ client: { Security_Policy: utf8('') } })).client.securityPolicy).toBe('Auto');
    expect(ok(legacyTree({ client: { Security_Policy: utf8('Basic256Sha256') } })).client.securityPolicy).toBe('Basic256Sha256');
    const levels: Record<string, string> = {
      '0x0': 'Untrusted',
      '0x0000': 'Untrusted',
      '0x1000': 'Low',
      '0x2000': 'Medium',
      '0x3000': 'High',
      '0x4000': 'System',
      '0x5000': 'ProtectedProcess',
    };
    for (const [hex, name] of Object.entries(levels)) {
      expect(ok(legacyTree({ pipe: { Min_Integrity_Level: utf8(hex) } })).pipe.minIntegrityLevel).toBe(name);
    }
    expect(ok(legacyTree({ pipe: { Min_Integrity_Level: utf8('low') } })).pipe.minIntegrityLevel).toBe('Low');
  });

  it('treats inbound rate limit 0 and -1 as no cap', () => {
    expect(ok(legacyTree({ pipe: { Inbound_Rate_Limit: i32(0) } })).pipe.inboundRateLimitHz).toBeUndefined();
    expect(ok(legacyTree({ pipe: { Inbound_Rate_Limit: i32(-1) } })).pipe.inboundRateLimitHz).toBeUndefined();
    expect(ok(legacyTree({ pipe: { Inbound_Rate_Limit: i32(30) } })).pipe.inboundRateLimitHz).toBe(30);
  });

  it('splits a trigger into Start and Stop rules with sequential ids in label order', () => {
    const c = ok(
      legacyTree({
        triggers: triggersTree({
          'B/x': {
            Signal: utf8('a/b'), Start_Value: utf8('1'), Stop_Value: utf8('2'), Rule_Enabled: i32(1),
            Cooldown_Period: i32(40), Max_Fires_Per_Job: i32(2), Case_Sensitivity: utf8('Contains'),
            Event: utf8('E'), Subsystem: utf8('S'), Component: utf8('C'), ID: utf8('t1'), Trigger_Label: utf8('B/x'),
          },
          A: { Signal: utf8('s'), Start_Value: utf8('go'), Stop_Value: utf8('') },
        }),
      }),
    );
    expect(c.triggers.rules.map((r) => [r.id, r.label, r.action])).toEqual([
      [1, 'A - Start', 'CaptureStart'],
      [2, 'B_x - Start', 'CaptureStart'],
      [3, 'B_x - Stop', 'CaptureStop'],
    ]);
    const [a, bStart, bStop] = c.triggers.rules;
    expect(a).toMatchObject({ enabled: false, cooldownMs: 0, combinator: 'And', priority: 0, description: '' });
    expect(a.maxFiresPerJob).toBeUndefined();
    expect(bStart).toMatchObject({ enabled: true, cooldownMs: 40, maxFiresPerJob: 2 });
    expect(bStop.conditions).toEqual([
      {
        id: 1,
        label: 'a_b',
        enabled: true,
        event: 'E',
        subsystem: 'S',
        component: 'C',
        signal: 'a/b',
        evaluation: { mode: 'String', value: '2', caseSensitivity: 'Contains' },
        extra: {},
      },
    ]);
    expect(a.conditions[0].evaluation).toEqual({ mode: 'String', value: 'go', caseSensitivity: 'Exact' });
  });

  it('reports skipped sides as warnings', () => {
    const c = ok(
      legacyTree({
        triggers: triggersTree({
          NoStop: { Signal: utf8('s'), Start_Value: utf8('1'), Stop_Value: utf8('') },
          NoSignal: { Signal: utf8(''), Start_Value: utf8('1'), Stop_Value: utf8('2') },
        }),
      }),
    );
    expect(c.triggers.rules.map((r) => r.label)).toEqual(['NoStop - Start']);
    expect(c.warnings).toEqual([
      'Triggers/NoSignal: skipped rule "NoSignal - Start": legacy trigger has no signal or value for that action',
      'Triggers/NoSignal: skipped rule "NoSignal - Stop": legacy trigger has no signal or value for that action',
      'Triggers/NoStop: skipped rule "NoStop - Stop": legacy trigger has no signal or value for that action',
    ]);
  });

  it('keeps unknown attributes in extra (trigger-level ones on each rule)', () => {
    const c = ok(
      legacyTree({
        client: { Client_Future: i32(1) },
        pipe: { Pipe_Future: utf8('p') },
        triggers: triggersTree(
          { T: { Signal: utf8('s'), Start_Value: utf8('1'), Stop_Value: utf8('2'), Trig_Future: i64(5n) } },
          { Triggers_Future: f64(2.5) },
        ),
      }),
    );
    expect(c.client.extra).toEqual({ Client_Future: i32(1) });
    expect(c.pipe.extra).toEqual({ Pipe_Future: utf8('p') });
    expect(c.triggers.extra).toEqual({ Triggers_Future: f64(2.5) });
    expect(c.triggers.rules.map((r) => r.extra)).toEqual([{ Trig_Future: i64(5n) }, { Trig_Future: i64(5n) }]);
    const packed = pack(c);
    if (!packed.ok) throw new Error(packed.error.message);
    expect(packed.value.children.Trigger.children['T - Stop'].attrs.Trig_Future).toEqual(i64(5n));
  });

  it('rejects invalid legacy data with a path', () => {
    const err = (tree: ExtensionTree) => {
      const r = unpack(tree);
      if (r.ok) throw new Error('expected an error');
      return { code: r.error.code, path: r.error.path };
    };
    const noUrl = legacyTree();
    delete noUrl.children.Client.attrs.Server_URL;
    expect(err(noUrl)).toEqual({ code: 'Validation', path: 'Client/@Server_URL' });
    expect(err(legacyTree({ client: { Auth_Mode: utf8('Nope') } }))).toEqual({ code: 'Validation', path: 'Client/@Auth_Mode' });
    expect(err(legacyTree({ client: { Sampling_Interval: i64(-5n) } }))).toEqual({ code: 'Validation', path: 'Client/@Sampling_Interval' });
    expect(err(legacyTree({ client: { Publish_Interval: f64(1.5) } }))).toEqual({ code: 'Validation', path: 'Client/@Publish_Interval' });
    expect(err(legacyTree({ client: { Publish_Interval: utf8('5') } }))).toEqual({ code: 'Validation', path: 'Client/@Publish_Interval' });
    expect(err(legacyTree({ pipe: { Pipe_Enabled: i32(2) } }))).toEqual({ code: 'Validation', path: 'Pipe/@Pipe_Enabled' });
    expect(err(legacyTree({ pipe: { Pipe_Name: utf8('x') } }))).toEqual({ code: 'Validation', path: 'Pipe/@Pipe_Name' });
    expect(err(legacyTree({ triggers: triggersTree({}, { Triggers_Enabled: f64(0.5) }) }))).toEqual({
      code: 'Validation',
      path: 'Triggers/@Triggers_Enabled',
    });
    expect(err(legacyTree({ triggers: triggersTree({ T: { Rule_Enabled: i32(3) } }) }))).toEqual({
      code: 'Validation',
      path: 'Triggers/T/@Rule_Enabled',
    });
    expect(err(legacyTree({ triggers: triggersTree({ T: { Case_Sensitivity: utf8('x') } }) }))).toEqual({
      code: 'Validation',
      path: 'Triggers/T/@Case_Sensitivity',
    });
    const noPipe = legacyTree();
    delete noPipe.children.Pipe;
    expect(err(noPipe)).toEqual({ code: 'Validation', path: 'Pipe' });
  });
});
