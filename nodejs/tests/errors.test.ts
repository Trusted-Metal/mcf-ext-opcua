import { describe, expect, it } from 'vitest';
import { pack, unpack, type OpcuaConfig, type Result, type ExtensionError } from '../src/index.js';
import { group, richModel, richTree } from './helpers/fixtures.js';

function expectErr(r: Result<unknown, ExtensionError>, code: string, path: string): void {
  expect(r.ok).toBe(false);
  if (r.ok) return;
  expect({ code: r.error.code, path: r.error.path }).toEqual({ code, path });
}

describe('unpack validation (v1)', () => {
  it('rich fixture is valid', () => {
    expect(unpack(richTree()).ok).toBe(true);
  });

  it('missing strict Client attribute', () => {
    const t = richTree();
    delete group(t, 'Client').attrs.Server_URL;
    expectErr(unpack(t), 'Validation', 'Client/@Server_URL');
  });

  it('missing strict Pipe attribute', () => {
    const t = richTree();
    delete group(t, 'Pipe').attrs.Pipe_Name;
    expectErr(unpack(t), 'Validation', 'Pipe/@Pipe_Name');
  });

  it('missing strict rule and condition attributes', () => {
    const t = richTree();
    delete group(t, 'Trigger', 'Job Stop').attrs.Action;
    expectErr(unpack(t), 'Validation', 'Trigger/Job Stop/@Action');

    const t2 = richTree();
    delete group(t2, 'Trigger', 'Job Start', 'Laser On').attrs.Value;
    expectErr(unpack(t2), 'Validation', 'Trigger/Job Start/Laser On/@Value');
  });

  it('missing Client / Trigger group', () => {
    const t = richTree();
    delete t.children.Client;
    expectErr(unpack(t), 'Validation', 'Client');
    const t2 = richTree();
    delete t2.children.Trigger;
    expectErr(unpack(t2), 'Validation', 'Trigger');
  });

  it('bad enum values', () => {
    const t = richTree();
    group(t, 'Client').attrs.Auth_Mode.value = 'Kerberos';
    expectErr(unpack(t), 'Validation', 'Client/@Auth_Mode');

    const t2 = richTree();
    group(t2, 'Trigger', 'Job Start', 'Laser On').attrs.Case_Sensitivity.value = 'Fuzzy';
    expectErr(unpack(t2), 'Validation', 'Trigger/Job Start/Laser On/@Case_Sensitivity');

    const t3 = richTree();
    group(t3, 'Pipe').attrs.User_Access_Level.value = 'currentuseronly';
    expectErr(unpack(t3), 'Validation', 'Pipe/@User_Access_Level');

    const t4 = richTree();
    group(t4, 'Client').attrs.Queue_Policy.value = '';
    expectErr(unpack(t4), 'Validation', 'Client/@Queue_Policy');
  });

  it('flag value 2', () => {
    const t = richTree();
    group(t, 'Pipe').attrs.Pipe_Enabled.value = 2;
    expectErr(unpack(t), 'Validation', 'Pipe/@Pipe_Enabled');

    const t2 = richTree();
    group(t2, 'Trigger', 'Job Start', 'Laser On').attrs.Condition_Enabled.value = 2;
    expectErr(unpack(t2), 'Validation', 'Trigger/Job Start/Laser On/@Condition_Enabled');
  });

  it('wrong dtype', () => {
    const t = richTree();
    group(t, 'Client').attrs.Publish_Interval.dtype = 'i64';
    group(t, 'Client').attrs.Publish_Interval.value = 5n;
    expectErr(unpack(t), 'Validation', 'Client/@Publish_Interval');
  });

  it('rule_id 0 and condition_id 0', () => {
    const t = richTree();
    group(t, 'Trigger', 'Job Start').attrs.Rule_Id.value = 0;
    expectErr(unpack(t), 'Validation', 'Trigger/Job Start/@Rule_Id');

    const t2 = richTree();
    group(t2, 'Trigger', 'Job Start', 'Laser On').attrs.Condition_Id.value = 0;
    expectErr(unpack(t2), 'Validation', 'Trigger/Job Start/Laser On/@Condition_Id');
  });

  it('duplicate rule id and condition id', () => {
    const t = richTree();
    group(t, 'Trigger', 'Job Stop').attrs.Rule_Id.value = 1;
    expectErr(unpack(t), 'Validation', 'Trigger/Job Stop/@Rule_Id');

    const t2 = richTree();
    group(t2, 'Trigger', 'Job Start', 'Oxygen High').attrs.Condition_Id.value = 1;
    expectErr(unpack(t2), 'Validation', 'Trigger/Job Start/Oxygen High/@Condition_Id');
  });

  it('label rules: "/", NUL, empty, and group/attr mismatch', () => {
    const t = richTree();
    const g = group(t, 'Trigger');
    g.children['a/b'] = g.children['Job Stop'];
    delete g.children['Job Stop'];
    g.children['a/b'].attrs.Rule_Label.value = 'a/b';
    expectErr(unpack(t), 'Validation', 'Trigger/a/b');

    const t2 = richTree();
    const g2 = group(t2, 'Trigger');
    g2.children['x\0y'] = g2.children['Job Stop'];
    delete g2.children['Job Stop'];
    expectErr(unpack(t2), 'Validation', 'Trigger/x\0y');

    const t3 = richTree();
    group(t3, 'Trigger', 'Job Stop').attrs.Rule_Label.value = 'Other';
    expectErr(unpack(t3), 'Validation', 'Trigger/Job Stop/@Rule_Label');

    const t4 = richTree();
    const g4 = group(t4, 'Trigger', 'Job Start');
    g4.children['bad/label'] = g4.children['Laser On'];
    delete g4.children['Laser On'];
    expectErr(unpack(t4), 'Validation', 'Trigger/Job Start/bad/label');
  });

  it('bad Pipe_Name', () => {
    const t = richTree();
    group(t, 'Pipe').attrs.Pipe_Name.value = 'opc_ua_client_pipe';
    expectErr(unpack(t), 'Validation', 'Pipe/@Pipe_Name');
    const t2 = richTree();
    group(t2, 'Pipe').attrs.Pipe_Name.value = '\\\\.\\pipe\\';
    expectErr(unpack(t2), 'Validation', 'Pipe/@Pipe_Name');
  });

  it('bad sampling interval -2', () => {
    const t = richTree();
    group(t, 'Client').attrs.Sampling_Interval.value = -2;
    expectErr(unpack(t), 'Validation', 'Client/@Sampling_Interval');
  });

  it('Numeric condition with non-numeric value', () => {
    for (const bad of ['abc', '', '0x10', 'NaN', 'Infinity', '1e999', ' 5']) {
      const t = richTree();
      group(t, 'Trigger', 'Job Start', 'Oxygen High').attrs.Value.value = bad;
      expectErr(unpack(t), 'Validation', 'Trigger/Job Start/Oxygen High/@Value');
    }
  });

  it('unknown major version', () => {
    const t = richTree();
    t.attrs.Opcua_Schema_Version.value = '2.0';
    expectErr(unpack(t), 'UnsupportedSchemaVersion', '@Opcua_Schema_Version');
  });

  it('malformed version attribute', () => {
    const t = richTree();
    t.attrs.Opcua_Schema_Version.value = 'one';
    expectErr(unpack(t), 'Validation', '@Opcua_Schema_Version');
  });

  it('unexpected groups and datasets are rejected, not dropped', () => {
    const t = richTree();
    t.children.Surprise = { attrs: {}, children: {}, datasets: {} };
    expectErr(unpack(t), 'Validation', 'Surprise');

    const t2 = richTree();
    group(t2, 'Client').children.Nested = { attrs: {}, children: {}, datasets: {} };
    expectErr(unpack(t2), 'Validation', 'Client/Nested');
  });
});

describe('pack validation', () => {
  const mutate = (fn: (c: OpcuaConfig) => void): Result<unknown, ExtensionError> => {
    const c = richModel();
    fn(c);
    return pack(c);
  };

  it('valid rich model packs', () => {
    expect(pack(richModel()).ok).toBe(true);
  });

  it('bad enum', () => {
    expectErr(
      mutate((c) => {
        (c.client as { authMode: string }).authMode = 'Nope';
      }),
      'Validation',
      'Client/@Auth_Mode',
    );
  });

  it('duplicate rule label, rule id, condition label', () => {
    expectErr(mutate((c) => { c.triggers.rules[1].label = 'Job Start'; }), 'Validation', 'Trigger/Job Start');
    expectErr(mutate((c) => { c.triggers.rules[1].id = 1; }), 'Validation', 'Trigger/Job Stop/@Rule_Id');
    expectErr(
      mutate((c) => { c.triggers.rules[0].conditions[1].label = 'Laser On'; }),
      'Validation',
      'Trigger/Job Start/Laser On',
    );
    expectErr(
      mutate((c) => { c.triggers.rules[0].conditions[1].id = 1; }),
      'Validation',
      'Trigger/Job Start/Oxygen High/@Condition_Id',
    );
  });

  it('"/" and NUL in labels', () => {
    expectErr(mutate((c) => { c.triggers.rules[0].label = 'a/b'; }), 'Validation', 'Trigger/a/b');
    expectErr(mutate((c) => { c.triggers.rules[0].label = 'a\0b'; }), 'Validation', 'Trigger/a\0b');
    expectErr(mutate((c) => { c.triggers.rules[0].label = ''; }), 'Validation', 'Trigger/');
    expectErr(
      mutate((c) => { c.triggers.rules[0].conditions[0].label = 'x/y'; }),
      'Validation',
      'Trigger/Job Start/x/y',
    );
  });

  it('ids must be >= 1', () => {
    expectErr(mutate((c) => { c.triggers.rules[0].id = 0; }), 'Validation', 'Trigger/Job Start/@Rule_Id');
    expectErr(
      mutate((c) => { c.triggers.rules[0].conditions[0].id = 0; }),
      'Validation',
      'Trigger/Job Start/Laser On/@Condition_Id',
    );
  });

  it('bad Pipe_Name', () => {
    expectErr(mutate((c) => { c.pipe.pipeName = 'nope'; }), 'Validation', 'Pipe/@Pipe_Name');
  });

  it('bad sampling interval', () => {
    for (const ms of [-2, 0, 1.5, Number.NaN]) {
      expectErr(
        mutate((c) => { c.client.samplingInterval = { kind: 'Ms', ms }; }),
        'Validation',
        'Client/@Sampling_Interval',
      );
    }
  });

  it('Numeric condition without finite threshold', () => {
    for (const threshold of [Number.NaN, Infinity, -Infinity]) {
      expectErr(
        mutate((c) => {
          const e = c.triggers.rules[0].conditions[1].evaluation;
          if (e.mode === 'Numeric') e.threshold = threshold;
        }),
        'Validation',
        'Trigger/Job Start/Oxygen High/@Value',
      );
    }
  });

  it('counts must be non-negative integers', () => {
    expectErr(mutate((c) => { c.client.publishInterval = -1; }), 'Validation', 'Client/@Publish_Interval');
    expectErr(mutate((c) => { c.client.bfsMaxDepth = 1.5; }), 'Validation', 'Client/@BFS_Max_Depth');
    expectErr(mutate((c) => { c.pipe.bufferSize = 2 ** 32; }), 'Validation', 'Pipe/@Buffer_Size');
    expectErr(mutate((c) => { c.triggers.rules[0].cooldownMs = -5; }), 'Validation', 'Trigger/Job Start/@Cooldown_Period');
    expectErr(mutate((c) => { c.triggers.rules[0].priority = Number.NaN; }), 'Validation', 'Trigger/Job Start/@Priority');
  });

  it('optional numbers must be positive when present', () => {
    expectErr(mutate((c) => { c.pipe.inboundRateLimitHz = 0; }), 'Validation', 'Pipe/@Inbound_Rate_Limit');
    expectErr(mutate((c) => { c.triggers.stopCeilingLayers = -1; }), 'Validation', 'Trigger/@Trigger_Stop_Ceiling_Layers');
    expectErr(mutate((c) => { c.triggers.rules[0].maxFiresPerJob = 0; }), 'Validation', 'Trigger/Job Start/@Max_Fires_Per_Job');
  });

  it('root node kinds must round-trip', () => {
    expectErr(mutate((c) => { c.client.rootNode = { kind: 'NodeId', value: 'no-equals' }; }), 'Validation', 'Client/@Root_Node');
    expectErr(mutate((c) => { c.client.rootNode = { kind: 'BrowseName', value: 'a=b' }; }), 'Validation', 'Client/@Root_Node');
    expectErr(mutate((c) => { c.client.rootNode = { kind: 'BrowseName', value: '' }; }), 'Validation', 'Client/@Root_Node');
  });

  it('extra attributes may not shadow known attributes or be malformed', () => {
    expectErr(
      mutate((c) => { c.client.extra.Server_URL = { dtype: 'utf8', value: 'x' }; }),
      'Validation',
      'Client/@Server_URL',
    );
    expectErr(
      mutate((c) => { c.extra.Opcua_Schema_Version = { dtype: 'utf8', value: '9.9' }; }),
      'Validation',
      '@Opcua_Schema_Version',
    );
    expectErr(
      mutate((c) => { c.pipe.extra.Bad = { dtype: 'u32', value: 1n }; }),
      'Validation',
      'Pipe/@Bad',
    );
    expectErr(
      mutate((c) => { c.pipe.extra['a/b'] = { dtype: 'u32', value: 1 }; }),
      'Validation',
      'Pipe/@a/b',
    );
  });

  it('flags must be booleans', () => {
    expectErr(
      mutate((c) => { (c.pipe as { enabled: unknown }).enabled = 1; }),
      'Validation',
      'Pipe/@Pipe_Enabled',
    );
  });
});
