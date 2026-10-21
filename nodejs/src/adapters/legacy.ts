import type { ExtensionAttr, ExtensionTree } from '../contract.js';
import { joinPath, validation } from '../errors.js';
import {
  assertLeaf,
  checkValue,
  readFields,
  requireChild,
  spec,
  type FieldSpec,
} from '../fields.js';
import type { ClientConfig, ExtraAttrs, OpcuaConfig, PipeConfig, Rule, TriggerConfig } from '../model.js';
import { CLIENT_FIELDS, CONDITION_FIELDS, PIPE_FIELDS, RULE_FIELDS, TRIGGER_FIELDS } from '../schema-v1.js';
import { optionalFromRaw, packV1, rootNodeFromRaw, samplingFromRaw } from './v1.js';

// Layout written by core's typed OPC UA support (no schema version attribute). Port of
// rdt-core's config-import-opcua-legacy.util.ts plus the legacy vocabulary mapping.

const CLIENT = 'Client';
const PIPE = 'Pipe';
const TRIGGERS = 'Triggers';

const LEGACY_INTEGRITY_LEVELS: Record<string, string> = {
  '0x0': 'Untrusted',
  '0x0000': 'Untrusted',
  '0x1000': 'Low',
  '0x2000': 'Medium',
  '0x3000': 'High',
  '0x4000': 'System',
  '0x5000': 'ProtectedProcess',
};

const CLIENT_KNOWN = [
  'Server_URL', 'Auth_Mode', 'Security_Mode', 'Security_Policy', 'BFS_Max_Depth', 'Publish_Interval',
  'Sampling_Interval', 'Session_Timeout', 'Keep_Alive_Count', 'Lifetime_Count', 'Machine_Profile',
  'Queue_Policy', 'Queue_Size_Data_Change', 'Queue_Size_Events', 'Reconnect_Interval', 'Root_Node',
  'Sync_Loop_Interval_Initial', 'Sync_Loop_Interval_Settled',
];
const PIPE_KNOWN = [
  'Pipe_Enabled', 'Buffer_Size', 'Configure_Client', 'Inbound_Rate_Limit', 'Max_Inbound_Message_Size',
  'Min_Integrity_Level', 'Pipe_Name', 'User_Access_Level',
];
const TRIGGERS_KNOWN = ['Triggers_Enabled', 'Trigger_Stop_Ceiling_Layers'];
// ID and Trigger_Label are dropped: rule ids are assigned and the group name is the label.
const TRIGGER_KNOWN = [
  'ID', 'Signal', 'Subsystem', 'Rule_Enabled', 'Start_Value', 'Stop_Value', 'Case_Sensitivity', 'Component',
  'Cooldown_Period', 'Event', 'Max_Fires_Per_Job', 'Trigger_Label',
];

/** Typed access to legacy attributes: integers may arrive as any integral numeric dtype. */
class LegacyGroup {
  constructor(
    private readonly attrs: Record<string, ExtensionAttr>,
    private readonly path: string,
  ) {}

  private scalar(name: string): ExtensionAttr | undefined {
    const a = this.attrs[name];
    if (a !== undefined && Array.isArray(a.value)) {
      throw validation(joinPath(this.path, `@${name}`), 'expected a scalar, got an array');
    }
    return a;
  }

  /** `def` undefined = required. */
  str(name: string, def?: string): string {
    const a = this.scalar(name);
    if (a === undefined) {
      if (def === undefined) throw validation(joinPath(this.path, `@${name}`), 'missing required attribute');
      return def;
    }
    if (a.dtype !== 'utf8') throw validation(joinPath(this.path, `@${name}`), `expected dtype utf8, got ${a.dtype}`);
    return a.value as string;
  }

  int(name: string, def?: number): number {
    const a = this.scalar(name);
    const here = joinPath(this.path, `@${name}`);
    if (a === undefined) {
      if (def === undefined) throw validation(here, 'missing required attribute');
      return def;
    }
    if (a.dtype === 'utf8') throw validation(here, 'expected a numeric attribute, got utf8');
    const v = a.value as number | bigint;
    const n = typeof v === 'bigint' ? Number(v) : v;
    if (typeof v === 'bigint' && !Number.isSafeInteger(n)) throw validation(here, `${v} is out of range`);
    if (!Number.isInteger(n)) throw validation(here, `expected an integral value, got ${v}`);
    return n;
  }

  optInt(name: string): number | undefined {
    return this.attrs[name] === undefined ? undefined : this.int(name);
  }

  /** Attribute values validated against the v1 field spec, so legacy data meets the same rules. */
  checked(f: FieldSpec, value: string | number): string | number {
    return checkValue(f, value, this.path);
  }
}

function extraOf(attrs: Record<string, ExtensionAttr>, known: readonly string[], path: string): ExtraAttrs {
  return readFields(attrs, [], path, known).extra;
}

function unpackClient(group: ExtensionTree): ClientConfig {
  assertLeaf(group, CLIENT);
  const g = new LegacyGroup(group.attrs, CLIENT);
  const field = (name: string) => spec(CLIENT_FIELDS, name);
  const str = (name: string, def?: string) => g.checked(field(name), g.str(name, def)) as string;
  const num = (name: string, def?: number) => g.checked(field(name), g.int(name, def)) as number;

  const profile = g.str('Machine_Profile', '');
  const policy = g.str('Security_Policy');
  const queue = g.str('Queue_Policy', 'DiscardOldest');
  return {
    serverUrl: str('Server_URL'),
    authMode: str('Auth_Mode') as ClientConfig['authMode'],
    ...(profile === '' ? {} : { machineProfile: g.checked(field('Machine_Profile'), profile) as 'Aconity' | 'Eos' }),
    securityMode: str('Security_Mode') as ClientConfig['securityMode'],
    securityPolicy: (policy === 'None' || policy === ''
      ? 'Auto'
      : g.checked(field('Security_Policy'), policy)) as ClientConfig['securityPolicy'],
    rootNode: rootNodeFromRaw(str('Root_Node', '')),
    samplingInterval: samplingFromRaw(num('Sampling_Interval'), `${CLIENT}/@Sampling_Interval`),
    publishInterval: num('Publish_Interval', 250),
    bfsMaxDepth: num('BFS_Max_Depth', 16),
    keepAliveCount: num('Keep_Alive_Count', 240),
    lifetimeCount: num('Lifetime_Count', 2400),
    queuePolicy: g.checked(field('Queue_Policy'), queue === 'DropOldest' ? 'DiscardOldest' : queue) as ClientConfig['queuePolicy'],
    queueSizeDataChange: num('Queue_Size_Data_Change', 10),
    queueSizeEvents: num('Queue_Size_Events', 10),
    reconnectInterval: num('Reconnect_Interval', 1000),
    sessionTimeout: num('Session_Timeout', 60000),
    syncLoopIntervalInitial: num('Sync_Loop_Interval_Initial', 1000),
    syncLoopIntervalSettled: num('Sync_Loop_Interval_Settled', 30000),
    extra: extraOf(group.attrs, CLIENT_KNOWN, CLIENT),
  };
}

function unpackPipe(group: ExtensionTree): PipeConfig {
  assertLeaf(group, PIPE);
  const g = new LegacyGroup(group.attrs, PIPE);
  const field = (name: string) => spec(PIPE_FIELDS, name);
  const num = (name: string, def: number) => g.checked(field(name), g.int(name, def)) as number;

  const level = g.str('Min_Integrity_Level', 'ProtectedProcess');
  const rate = optionalFromRaw(num('Inbound_Rate_Limit', 100), `${PIPE}/@Inbound_Rate_Limit`, true);
  return {
    enabled: num('Pipe_Enabled', 0) === 1,
    configureClient: num('Configure_Client', 0) === 1,
    bufferSize: num('Buffer_Size', 65536),
    ...(rate === undefined ? {} : { inboundRateLimitHz: rate }),
    maxInboundMessageSize: num('Max_Inbound_Message_Size', 4096),
    minIntegrityLevel: g.checked(
      field('Min_Integrity_Level'),
      LEGACY_INTEGRITY_LEVELS[level.toLowerCase()] ?? level,
    ) as PipeConfig['minIntegrityLevel'],
    pipeName: g.checked(field('Pipe_Name'), g.str('Pipe_Name', '\\\\.\\pipe\\opc_ua_client_pipe')) as string,
    userAccessLevel: g.checked(field('User_Access_Level'), g.str('User_Access_Level', 'CurrentUserOnly')) as PipeConfig['userAccessLevel'],
    extra: extraOf(group.attrs, PIPE_KNOWN, PIPE),
  };
}

function unpackTriggers(group: ExtensionTree | undefined, warnings: string[]): TriggerConfig {
  if (group === undefined) return { enabled: false, rules: [], extra: {} };
  if (Object.keys(group.datasets).length > 0) throw validation(joinPath(TRIGGERS, Object.keys(group.datasets)[0]), 'unexpected dataset');
  const g = new LegacyGroup(group.attrs, TRIGGERS);
  const ceiling = g.optInt('Trigger_Stop_Ceiling_Layers');
  const stop =
    ceiling === undefined
      ? undefined
      : optionalFromRaw(
          g.checked(spec(TRIGGER_FIELDS, 'Trigger_Stop_Ceiling_Layers'), ceiling) as number,
          `${TRIGGERS}/@Trigger_Stop_Ceiling_Layers`,
        );
  const enabled = g.checked(spec(TRIGGER_FIELDS, 'Triggers_Enabled'), g.int('Triggers_Enabled', 0)) === 1;

  const rules: Rule[] = [];
  let nextId = 1;
  for (const triggerLabel of Object.keys(group.children).sort()) {
    const trigger = group.children[triggerLabel];
    const path = joinPath(TRIGGERS, triggerLabel);
    assertLeaf(trigger, path);
    const t = new LegacyGroup(trigger.attrs, path);

    const label = triggerLabel.replace(/\//g, '_');
    const signal = t.str('Signal', '');
    const ruleEnabled = t.checked(spec(RULE_FIELDS, 'Rule_Enabled'), t.int('Rule_Enabled', 0)) === 1;
    const cooldown = t.checked(spec(RULE_FIELDS, 'Cooldown_Period'), t.int('Cooldown_Period', 0)) as number;
    const maxFires = optionalFromRaw(
      t.checked(spec(RULE_FIELDS, 'Max_Fires_Per_Job'), t.int('Max_Fires_Per_Job', 0)) as number,
      joinPath(path, '@Max_Fires_Per_Job'),
    );
    const cond = (name: string) => t.checked(spec(CONDITION_FIELDS, name), t.str(name, name === 'Case_Sensitivity' ? 'Exact' : '')) as string;
    const event = cond('Event');
    const subsystem = cond('Subsystem');
    const component = cond('Component');
    const caseSensitivity = cond('Case_Sensitivity');
    const sides = [
      { action: 'CaptureStart', suffix: 'Start', value: t.str('Start_Value', '') },
      { action: 'CaptureStop', suffix: 'Stop', value: t.str('Stop_Value', '') },
    ] as const;

    for (const side of sides) {
      const ruleLabel = `${label} - ${side.suffix}`;
      if (!signal || !side.value) {
        warnings.push(`${path}: skipped rule "${ruleLabel}": legacy trigger has no signal or value for that action`);
        continue;
      }
      const conditionLabel = signal.replace(/\//g, '_');
      rules.push({
        id: nextId++,
        label: ruleLabel,
        description: '',
        action: side.action,
        combinator: 'And',
        enabled: ruleEnabled,
        priority: 0,
        cooldownMs: cooldown,
        ...(maxFires === undefined ? {} : { maxFiresPerJob: maxFires }),
        conditions: [
          {
            id: 1,
            label: conditionLabel,
            enabled: true,
            event,
            subsystem,
            component,
            signal,
            evaluation: {
              mode: 'String',
              value: side.value,
              caseSensitivity: caseSensitivity as 'Exact',
            },
            extra: {},
          },
        ],
        extra: extraOf(trigger.attrs, TRIGGER_KNOWN, path),
      });
    }
  }

  return {
    enabled,
    ...(stop === undefined ? {} : { stopCeilingLayers: stop }),
    rules,
    extra: extraOf(group.attrs, TRIGGERS_KNOWN, TRIGGERS),
  };
}

/** Converts the legacy layout to the model; the result is validated the same way `pack` validates. */
export function unpackLegacy(tree: ExtensionTree): OpcuaConfig {
  if (Object.keys(tree.datasets).length > 0) throw validation(Object.keys(tree.datasets)[0], 'unexpected dataset');
  for (const name of Object.keys(tree.children)) {
    if (name !== CLIENT && name !== PIPE && name !== TRIGGERS) throw validation(name, 'unexpected child group');
  }
  const warnings: string[] = [];
  const config: OpcuaConfig = {
    client: unpackClient(requireChild(tree, CLIENT)),
    pipe: unpackPipe(requireChild(tree, PIPE)),
    triggers: unpackTriggers(tree.children[TRIGGERS], warnings),
    extra: extraOf(tree.attrs, [], ''),
    warnings,
  };
  packV1(config);
  return config;
}
