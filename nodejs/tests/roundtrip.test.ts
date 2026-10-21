import { describe, expect, it } from 'vitest';
import { emptyTree, pack, unpack, type ExtensionTree, type OpcuaConfig } from '../src/index.js';
import { cloneTree } from './helpers/treeJson.js';

function richConfig(): OpcuaConfig {
  return {
    client: {
      serverUrl: 'opc.tcp://example:4840/srv',
      authMode: 'Certificate',
      machineProfile: 'Aconity',
      securityMode: 'Sign',
      securityPolicy: 'ECC_brainpoolP256r1',
      rootNode: { kind: 'BrowseName', value: 'Fleet' },
      samplingInterval: { kind: 'Ms', ms: 50 },
      publishInterval: 100,
      bfsMaxDepth: 4,
      keepAliveCount: 10,
      lifetimeCount: 30,
      queuePolicy: 'KeepOldest',
      queueSizeDataChange: 50,
      queueSizeEvents: 20,
      reconnectInterval: 2000,
      sessionTimeout: 45000,
      syncLoopIntervalInitial: 100,
      syncLoopIntervalSettled: 1000,
      extra: { Client_Utf8: { dtype: 'utf8', value: 'x' }, Client_U64: { dtype: 'u64', value: 2n ** 63n } },
    },
    pipe: {
      enabled: true,
      configureClient: false,
      bufferSize: 1024,
      inboundRateLimitHz: 60,
      maxInboundMessageSize: 2048,
      minIntegrityLevel: 'System',
      pipeName: '\\\\.\\pipe\\rt',
      userAccessLevel: 'CurrentUserAndAdmins',
      extra: { Pipe_F32: { dtype: 'f32', value: 0.5 }, Pipe_Arr: { dtype: 'utf8', value: ['a', 'b'] } },
    },
    triggers: {
      enabled: true,
      stopCeilingLayers: 2,
      extra: { Trig_I8: { dtype: 'i8', value: -3 } },
      rules: [
        {
          id: 1,
          label: 'Start A',
          description: 'first',
          action: 'CaptureStart',
          combinator: 'Or',
          enabled: true,
          priority: 5,
          cooldownMs: 250,
          maxFiresPerJob: 2,
          extra: { Rule_Extra: { dtype: 'i32', value: -1 } },
          conditions: [
            {
              id: 1,
              label: 'cond one',
              enabled: true,
              event: 'E',
              subsystem: 'S',
              component: 'C',
              signal: 'sig',
              evaluation: { mode: 'String', value: 'go', caseSensitivity: 'Contains' },
              extra: { Cond_Extra: { dtype: 'u16', value: 65535 } },
            },
            {
              id: 4,
              label: 'cond numeric',
              enabled: false,
              event: '',
              subsystem: '',
              component: '',
              signal: 'temp',
              evaluation: { mode: 'Numeric', threshold: -0.125, operator: 'LessThanOrEqual', epsilon: 1e-9 },
              extra: {},
            },
          ],
        },
        {
          id: 9,
          label: 'Stop A',
          description: '',
          action: 'CaptureStop',
          enabled: false,
          priority: 0,
          cooldownMs: 0,
          conditions: [],
          extra: {},
        },
      ],
    },
    extra: { Root_Extra: { dtype: 'i64', value: -9n } },
  };
}

const attr = (tree: ExtensionTree, ...path: string[]) => {
  const name = path.pop() as string;
  let g = tree;
  for (const p of path) g = g.children[p];
  return g.attrs[name];
};

describe('round trip', () => {
  it('pack then unpack deep-equals the original', () => {
    const config = richConfig();
    const packed = pack(config);
    expect(packed.ok).toBe(true);
    if (!packed.ok) return;
    const back = unpack(packed.value);
    expect(back).toEqual({ ok: true, value: config });
  });

  it('pack does not mutate or alias the input', () => {
    const config = richConfig();
    const snapshot = richConfig();
    const packed = pack(config);
    expect(config).toEqual(snapshot);
    if (!packed.ok) throw new Error('pack failed');
    const arr = packed.value.children.Pipe.attrs.Pipe_Arr.value as string[];
    arr.push('mutated');
    expect(config.pipe.extra.Pipe_Arr.value).toEqual(['a', 'b']);
  });

  it('is deterministic', () => {
    const a = pack(richConfig());
    const b = pack(richConfig());
    expect(JSON.stringify(a, (_k, v) => (typeof v === 'bigint' ? `${v}n` : v))).toBe(
      JSON.stringify(b, (_k, v) => (typeof v === 'bigint' ? `${v}n` : v)),
    );
    if (!a.ok) throw new Error('pack failed');
    expect(Object.keys(a.value.children.Client.attrs)).toEqual([...Object.keys(a.value.children.Client.attrs)].sort());
  });

  it('writes the documented dtypes', () => {
    const r = pack(richConfig());
    if (!r.ok) throw new Error('pack failed');
    const t = r.value;
    expect(t.attrs.Opcua_Schema_Version).toEqual({ dtype: 'utf8', value: '1.2' });
    expect(attr(t, 'Client', 'Server_URL').dtype).toBe('utf8');
    expect(attr(t, 'Client', 'Sampling_Interval')).toEqual({ dtype: 'i32', value: 50 });
    expect(attr(t, 'Client', 'Publish_Interval')).toEqual({ dtype: 'u32', value: 100 });
    expect(attr(t, 'Client', 'Security_Policy')).toEqual({ dtype: 'utf8', value: 'ECC_brainpoolP256r1' });
    expect(attr(t, 'Pipe', 'Pipe_Enabled')).toEqual({ dtype: 'u32', value: 1 });
    expect(attr(t, 'Pipe', 'Configure_Client')).toEqual({ dtype: 'u32', value: 0 });
    expect(attr(t, 'Pipe', 'Inbound_Rate_Limit')).toEqual({ dtype: 'i32', value: 60 });
    expect(attr(t, 'Trigger', 'Triggers_Enabled')).toEqual({ dtype: 'u32', value: 1 });
    expect(attr(t, 'Trigger', 'Trigger_Stop_Ceiling_Layers')).toEqual({ dtype: 'u32', value: 2 });
    expect(attr(t, 'Trigger', 'Start A', 'Rule_Id')).toEqual({ dtype: 'u32', value: 1 });
    expect(attr(t, 'Trigger', 'Start A', 'Combinator')).toEqual({ dtype: 'utf8', value: 'Or' });
    expect(attr(t, 'Trigger', 'Start A', 'Cooldown_Period')).toEqual({ dtype: 'u32', value: 250 });
    expect(attr(t, 'Trigger', 'Start A', 'cond numeric', 'Epsilon')).toEqual({ dtype: 'f64', value: 1e-9 });
    expect(attr(t, 'Trigger', 'Start A', 'cond numeric', 'Value')).toEqual({ dtype: 'utf8', value: '-0.125' });
    expect(attr(t, 'Trigger', 'Start A', 'cond numeric', 'Evaluation_Mode').value).toBe('Numeric');
    expect(attr(t, 'Trigger', 'Start A', 'cond one', 'Epsilon')).toEqual({ dtype: 'f64', value: 0 });
    expect(attr(t, 'Trigger', 'Start A', 'cond one', 'Comparison_Operator').value).toBe('Equal');
  });

  it('writes every defaulted attribute and the sentinel forms', () => {
    const r = pack(richConfig());
    if (!r.ok) throw new Error('pack failed');
    const stop = r.value.children.Trigger.children['Stop A'];
    expect(stop.attrs.Combinator).toEqual({ dtype: 'utf8', value: '' });
    expect(stop.attrs.Max_Fires_Per_Job).toEqual({ dtype: 'u32', value: 0 });
    expect(stop.attrs.Description).toEqual({ dtype: 'utf8', value: '' });
    expect(Object.keys(stop.attrs)).toHaveLength(9);

    const c = richConfig();
    delete c.client.machineProfile;
    delete c.pipe.inboundRateLimitHz;
    delete c.triggers.stopCeilingLayers;
    const r2 = pack(c);
    if (!r2.ok) throw new Error('pack failed');
    expect(r2.value.children.Client.attrs.Machine_Profile).toEqual({ dtype: 'utf8', value: '' });
    expect(r2.value.children.Pipe.attrs.Inbound_Rate_Limit).toEqual({ dtype: 'i32', value: 0 });
    expect(r2.value.children.Trigger.attrs.Trigger_Stop_Ceiling_Layers).toEqual({ dtype: 'u32', value: 0 });
    expect(unpack(r2.value)).toEqual({ ok: true, value: c });
  });

  it('keeps the dtype of unknown attributes', () => {
    const r = pack(richConfig());
    if (!r.ok) throw new Error('pack failed');
    expect(attr(r.value, 'Root_Extra')).toEqual({ dtype: 'i64', value: -9n });
    expect(attr(r.value, 'Client', 'Client_U64')).toEqual({ dtype: 'u64', value: 2n ** 63n });
    expect(attr(r.value, 'Pipe', 'Pipe_F32')).toEqual({ dtype: 'f32', value: 0.5 });
    expect(attr(r.value, 'Trigger', 'Trig_I8')).toEqual({ dtype: 'i8', value: -3 });
    expect(attr(r.value, 'Trigger', 'Start A', 'cond one', 'Cond_Extra')).toEqual({ dtype: 'u16', value: 65535 });
  });

  it('writes rules and conditions sorted by id', () => {
    const c = richConfig();
    c.triggers.rules.reverse();
    c.triggers.rules[1].conditions.reverse();
    const r = pack(c);
    if (!r.ok) throw new Error('pack failed');
    expect(Object.keys(r.value.children.Trigger.children)).toEqual(['Start A', 'Stop A']);
    expect(Object.keys(r.value.children.Trigger.children['Start A'].children)).toEqual(['cond one', 'cond numeric']);
  });

  it('unpack then pack reproduces a packed tree', () => {
    const first = pack(richConfig());
    if (!first.ok) throw new Error('pack failed');
    const model = unpack(cloneTree(first.value));
    if (!model.ok) throw new Error('unpack failed');
    expect(pack(model.value)).toEqual(first);
  });

  it('numeric thresholds use the shortest round-trip decimal', () => {
    for (const threshold of [700, 700.5, 1e21, 1e-7, -0.1 - 0.2, 123456789.123456789]) {
      const c = richConfig();
      const e = c.triggers.rules[0].conditions[1].evaluation;
      if (e.mode !== 'Numeric') throw new Error('fixture');
      e.threshold = threshold;
      const r = pack(c);
      if (!r.ok) throw new Error('pack failed');
      expect(attr(r.value, 'Trigger', 'Start A', 'cond numeric', 'Value').value).toBe(String(threshold));
      expect(unpack(r.value)).toEqual({ ok: true, value: c });
    }
  });

  it('emptyTree is a fresh empty tree', () => {
    expect(emptyTree()).toEqual({ attrs: {}, children: {}, datasets: {} });
    expect(emptyTree()).not.toBe(emptyTree());
  });
});
