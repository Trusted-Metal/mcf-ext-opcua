import { emptyTree, type ExtensionTree } from '../contract.js';
import { joinPath, validation } from '../errors.js';
import {
  assertLeaf,
  isLabel,
  readFields,
  requireChild,
  Values,
  writeFields,
  type RawValue,
} from '../fields.js';
import type {
  CaseSensitivity,
  ClientConfig,
  Combinator,
  ComparisonOperator,
  Condition,
  Evaluation,
  ExtraAttrs,
  OpcuaConfig,
  PipeConfig,
  RootNode,
  Rule,
  SamplingInterval,
  TriggerConfig,
} from '../model.js';
import {
  CLIENT_FIELDS,
  CONDITION_FIELDS,
  CURRENT_SCHEMA_VERSION,
  PIPE_FIELDS,
  RULE_FIELDS,
  SCHEMA_VERSION_ATTR,
  TRIGGER_FIELDS,
} from '../schema-v1.js';

const CLIENT = 'Client';
const PIPE = 'Pipe';
const TRIGGER = 'Trigger';
const I32_MAX = 2147483647;
const NUMERIC_VALUE = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;

function assertNoDatasets(tree: ExtensionTree, path: string): void {
  const name = Object.keys(tree.datasets)[0];
  if (name !== undefined) throw validation(joinPath(path, name), 'unexpected dataset');
}

// ---- shared disk <-> model scalar mappings (also used by the legacy adapter) ----

export function rootNodeFromRaw(raw: string): RootNode {
  if (raw === '') return { kind: 'AutoDiscover' };
  return raw.includes('=') ? { kind: 'NodeId', value: raw } : { kind: 'BrowseName', value: raw };
}

function rootNodeToRaw(node: RootNode, path: string): string {
  switch (node?.kind) {
    case 'AutoDiscover':
      return '';
    case 'NodeId':
      if (typeof node.value !== 'string' || !node.value.includes('=')) throw validation(path, 'NodeId must contain "="');
      return node.value;
    case 'BrowseName':
      if (typeof node.value !== 'string' || node.value === '' || node.value.includes('=')) {
        throw validation(path, 'BrowseName must be non-empty and must not contain "="');
      }
      return node.value;
    default:
      throw validation(path, 'invalid root node');
  }
}

export function samplingFromRaw(raw: number, path: string): SamplingInterval {
  if (raw === -1) return { kind: 'ServerNegotiated' };
  if (raw === 0) return { kind: 'Fastest' };
  if (raw > 0) return { kind: 'Ms', ms: raw };
  throw validation(path, `invalid sampling interval ${raw}; expected -1, 0 or a positive number of ms`);
}

function samplingToRaw(s: SamplingInterval, path: string): number {
  switch (s?.kind) {
    case 'ServerNegotiated':
      return -1;
    case 'Fastest':
      return 0;
    case 'Ms':
      if (!Number.isInteger(s.ms) || s.ms < 1 || s.ms > I32_MAX) {
        throw validation(path, `Ms must be an integer between 1 and ${I32_MAX}`);
      }
      return s.ms;
    default:
      throw validation(path, 'invalid sampling interval');
  }
}

/** Disk 0 (and -1 where `alsoNone`) means absent; other negatives are errors. */
export function optionalFromRaw(raw: number, path: string, alsoNone = false): number | undefined {
  if (raw === 0 || (alsoNone && raw === -1)) return undefined;
  if (raw < 0) throw validation(path, `invalid value ${raw}`);
  return raw;
}

function optionalToRaw(n: number | undefined, path: string): number {
  if (n === undefined) return 0;
  if (!Number.isInteger(n) || n < 1) throw validation(path, 'must be a positive integer when present');
  return n;
}

function flagToRaw(b: unknown, path: string): number {
  if (typeof b !== 'boolean') throw validation(path, 'expected a boolean');
  return b ? 1 : 0;
}

function optionalEnumToRaw(v: string | undefined, path: string): string {
  if (v === undefined) return '';
  if (v === '') throw validation(path, 'use undefined instead of an empty string');
  return v;
}

// ---- unpack ----

function unpackClient(group: ExtensionTree): ClientConfig {
  assertLeaf(group, CLIENT);
  const { values, extra } = readFields(group.attrs, CLIENT_FIELDS, CLIENT);
  const v = new Values(values);
  const profile = v.str('Machine_Profile');
  return {
    serverUrl: v.str('Server_URL'),
    authMode: v.str('Auth_Mode') as ClientConfig['authMode'],
    ...(profile === '' ? {} : { machineProfile: profile as 'Aconity' | 'Eos' }),
    securityMode: v.str('Security_Mode') as ClientConfig['securityMode'],
    securityPolicy: (v.str('Security_Policy') || 'Auto') as ClientConfig['securityPolicy'],
    rootNode: rootNodeFromRaw(v.str('Root_Node')),
    samplingInterval: samplingFromRaw(v.num('Sampling_Interval'), `${CLIENT}/@Sampling_Interval`),
    publishInterval: v.num('Publish_Interval'),
    bfsMaxDepth: v.num('BFS_Max_Depth'),
    keepAliveCount: v.num('Keep_Alive_Count'),
    lifetimeCount: v.num('Lifetime_Count'),
    queuePolicy: v.str('Queue_Policy') as ClientConfig['queuePolicy'],
    queueSizeDataChange: v.num('Queue_Size_Data_Change'),
    queueSizeEvents: v.num('Queue_Size_Events'),
    reconnectInterval: v.num('Reconnect_Interval'),
    sessionTimeout: v.num('Session_Timeout'),
    syncLoopIntervalInitial: v.num('Sync_Loop_Interval_Initial'),
    syncLoopIntervalSettled: v.num('Sync_Loop_Interval_Settled'),
    extra,
  };
}

function unpackPipe(group: ExtensionTree): PipeConfig {
  assertLeaf(group, PIPE);
  const { values, extra } = readFields(group.attrs, PIPE_FIELDS, PIPE);
  const v = new Values(values);
  const rate = optionalFromRaw(v.num('Inbound_Rate_Limit'), `${PIPE}/@Inbound_Rate_Limit`, true);
  return {
    enabled: v.flag('Pipe_Enabled'),
    configureClient: v.flag('Configure_Client'),
    bufferSize: v.num('Buffer_Size'),
    ...(rate === undefined ? {} : { inboundRateLimitHz: rate }),
    maxInboundMessageSize: v.num('Max_Inbound_Message_Size'),
    minIntegrityLevel: v.str('Min_Integrity_Level') as PipeConfig['minIntegrityLevel'],
    pipeName: v.str('Pipe_Name'),
    userAccessLevel: v.str('User_Access_Level') as PipeConfig['userAccessLevel'],
    extra,
  };
}

function unpackCondition(label: string, group: ExtensionTree, rulePath: string): Condition {
  const path = joinPath(rulePath, label);
  if (!isLabel(label)) throw validation(path, 'label must be non-empty and contain no "/" or NUL');
  assertLeaf(group, path);
  const { values, extra } = readFields(group.attrs, CONDITION_FIELDS, path);
  const v = new Values(values);
  if (v.str('Condition_Label') !== label) {
    throw validation(joinPath(path, '@Condition_Label'), `must equal the group name "${label}"`);
  }
  let evaluation: Evaluation;
  if (v.str('Evaluation_Mode') === 'Numeric') {
    const text = v.str('Value');
    if (!NUMERIC_VALUE.test(text) || !Number.isFinite(Number(text))) {
      throw validation(joinPath(path, '@Value'), `Numeric condition value "${text}" is not a finite number`);
    }
    evaluation = {
      mode: 'Numeric',
      threshold: Number(text),
      operator: v.str('Comparison_Operator') as ComparisonOperator,
      epsilon: v.num('Epsilon'),
    };
  } else {
    evaluation = {
      mode: 'String',
      value: v.str('Value'),
      caseSensitivity: v.str('Case_Sensitivity') as CaseSensitivity,
    };
  }
  return {
    id: v.num('Condition_Id'),
    label,
    enabled: v.flag('Condition_Enabled'),
    event: v.str('Event'),
    subsystem: v.str('Subsystem'),
    component: v.str('Component'),
    signal: v.str('Signal'),
    evaluation,
    extra,
  };
}

function unpackRule(label: string, group: ExtensionTree): Rule {
  const path = joinPath(TRIGGER, label);
  if (!isLabel(label)) throw validation(path, 'label must be non-empty and contain no "/" or NUL');
  assertNoDatasets(group, path);
  const { values, extra } = readFields(group.attrs, RULE_FIELDS, path);
  const v = new Values(values);
  if (v.str('Rule_Label') !== label) throw validation(joinPath(path, '@Rule_Label'), `must equal the group name "${label}"`);

  const conditions: Condition[] = [];
  const ids = new Set<number>();
  for (const [name, child] of Object.entries(group.children)) {
    const c = unpackCondition(name, child, path);
    if (ids.has(c.id)) throw validation(joinPath(joinPath(path, name), '@Condition_Id'), `duplicate condition id ${c.id}`);
    ids.add(c.id);
    conditions.push(c);
  }
  conditions.sort((a, b) => a.id - b.id);

  const combinator = v.str('Combinator');
  const maxFires = optionalFromRaw(v.num('Max_Fires_Per_Job'), joinPath(path, '@Max_Fires_Per_Job'));
  return {
    id: v.num('Rule_Id'),
    label,
    description: v.str('Description'),
    action: v.str('Action') as Rule['action'],
    ...(combinator === '' ? {} : { combinator: combinator as Combinator }),
    enabled: v.flag('Rule_Enabled'),
    priority: v.num('Priority'),
    cooldownMs: v.num('Cooldown_Period'),
    ...(maxFires === undefined ? {} : { maxFiresPerJob: maxFires }),
    conditions,
    extra,
  };
}

function unpackTriggers(group: ExtensionTree): TriggerConfig {
  assertNoDatasets(group, TRIGGER);
  const { values, extra } = readFields(group.attrs, TRIGGER_FIELDS, TRIGGER);
  const v = new Values(values);
  const rules: Rule[] = [];
  const ids = new Set<number>();
  for (const [name, child] of Object.entries(group.children)) {
    const r = unpackRule(name, child);
    if (ids.has(r.id)) throw validation(joinPath(joinPath(TRIGGER, name), '@Rule_Id'), `duplicate rule id ${r.id}`);
    ids.add(r.id);
    rules.push(r);
  }
  rules.sort((a, b) => a.id - b.id);
  const ceiling = optionalFromRaw(v.num('Trigger_Stop_Ceiling_Layers'), `${TRIGGER}/@Trigger_Stop_Ceiling_Layers`);
  return {
    enabled: v.flag('Triggers_Enabled'),
    ...(ceiling === undefined ? {} : { stopCeilingLayers: ceiling }),
    rules,
    extra,
  };
}

/** Unpacks any 1.x tree; unknown attributes go to `extra`, unknown groups and datasets are errors. */
export function unpackV1(tree: ExtensionTree): OpcuaConfig {
  assertNoDatasets(tree, '');
  for (const name of Object.keys(tree.children)) {
    if (name !== CLIENT && name !== PIPE && name !== TRIGGER) throw validation(name, 'unexpected child group');
  }
  const { extra } = readFields(tree.attrs, [], '', [SCHEMA_VERSION_ATTR]);
  return {
    client: unpackClient(requireChild(tree, CLIENT)),
    pipe: unpackPipe(requireChild(tree, PIPE)),
    triggers: unpackTriggers(requireChild(tree, TRIGGER)),
    extra,
  };
}

// ---- pack ----

function packClient(c: ClientConfig): ExtensionTree {
  const p = (name: string) => joinPath(CLIENT, `@${name}`);
  const policy: string = c.securityPolicy;
  if (policy === '') throw validation(p('Security_Policy'), 'use "Auto" instead of an empty string');
  const values: Record<string, RawValue> = {
    Auth_Mode: c.authMode,
    BFS_Max_Depth: c.bfsMaxDepth,
    Keep_Alive_Count: c.keepAliveCount,
    Lifetime_Count: c.lifetimeCount,
    Machine_Profile: optionalEnumToRaw(c.machineProfile, p('Machine_Profile')),
    Publish_Interval: c.publishInterval,
    Queue_Policy: c.queuePolicy,
    Queue_Size_Data_Change: c.queueSizeDataChange,
    Queue_Size_Events: c.queueSizeEvents,
    Reconnect_Interval: c.reconnectInterval,
    Root_Node: rootNodeToRaw(c.rootNode, p('Root_Node')),
    Sampling_Interval: samplingToRaw(c.samplingInterval, p('Sampling_Interval')),
    Security_Mode: c.securityMode,
    Security_Policy: policy === 'Auto' ? '' : policy,
    Server_URL: c.serverUrl,
    Session_Timeout: c.sessionTimeout,
    Sync_Loop_Interval_Initial: c.syncLoopIntervalInitial,
    Sync_Loop_Interval_Settled: c.syncLoopIntervalSettled,
  };
  return { ...emptyTree(), attrs: writeFields(values, CLIENT_FIELDS, c.extra, CLIENT) };
}

function packPipe(c: PipeConfig): ExtensionTree {
  const p = (name: string) => joinPath(PIPE, `@${name}`);
  const values: Record<string, RawValue> = {
    Buffer_Size: c.bufferSize,
    Configure_Client: flagToRaw(c.configureClient, p('Configure_Client')),
    Inbound_Rate_Limit: optionalToRaw(c.inboundRateLimitHz, p('Inbound_Rate_Limit')),
    Max_Inbound_Message_Size: c.maxInboundMessageSize,
    Min_Integrity_Level: c.minIntegrityLevel,
    Pipe_Enabled: flagToRaw(c.enabled, p('Pipe_Enabled')),
    Pipe_Name: c.pipeName,
    User_Access_Level: c.userAccessLevel,
  };
  return { ...emptyTree(), attrs: writeFields(values, PIPE_FIELDS, c.extra, PIPE) };
}

function packCondition(c: Condition, rulePath: string): ExtensionTree {
  const path = joinPath(rulePath, String(c.label));
  const p = (name: string) => joinPath(path, `@${name}`);
  const e = c.evaluation;
  let eval_: Record<string, RawValue>;
  if (e?.mode === 'Numeric') {
    if (typeof e.threshold !== 'number' || !Number.isFinite(e.threshold)) {
      throw validation(p('Value'), 'Numeric threshold must be a finite number');
    }
    eval_ = {
      Evaluation_Mode: 'Numeric',
      Value: String(e.threshold),
      Case_Sensitivity: 'Exact',
      Comparison_Operator: e.operator,
      Epsilon: e.epsilon,
    };
  } else if (e?.mode === 'String') {
    eval_ = {
      Evaluation_Mode: 'String',
      Value: e.value,
      Case_Sensitivity: e.caseSensitivity,
      Comparison_Operator: 'Equal',
      Epsilon: 0,
    };
  } else {
    throw validation(p('Evaluation_Mode'), 'invalid evaluation mode');
  }
  const values: Record<string, RawValue> = {
    Condition_Id: c.id,
    Condition_Label: c.label,
    Condition_Enabled: flagToRaw(c.enabled, p('Condition_Enabled')),
    Signal: c.signal,
    Event: c.event,
    Subsystem: c.subsystem,
    Component: c.component,
    ...eval_,
  };
  return { ...emptyTree(), attrs: writeFields(values, CONDITION_FIELDS, c.extra, path) };
}

function uniqueSorted<T extends { id: number; label: string }>(
  items: readonly T[],
  parentPath: string,
  idAttr: string,
  kind: string,
): T[] {
  const labels = new Set<string>();
  const ids = new Set<number>();
  for (const it of items) {
    const path = joinPath(parentPath, String(it.label));
    if (!isLabel(it.label)) throw validation(path, `${kind} label must be non-empty and contain no "/" or NUL`);
    if (labels.has(it.label)) throw validation(path, `duplicate ${kind} label "${it.label}"`);
    labels.add(it.label);
    if (ids.has(it.id)) throw validation(joinPath(path, `@${idAttr}`), `duplicate ${kind} id ${it.id}`);
    ids.add(it.id);
  }
  return [...items].sort((a, b) => a.id - b.id);
}

function packRule(r: Rule): ExtensionTree {
  const path = joinPath(TRIGGER, r.label);
  const p = (name: string) => joinPath(path, `@${name}`);
  const values: Record<string, RawValue> = {
    Rule_Id: r.id,
    Rule_Label: r.label,
    Action: r.action,
    Rule_Enabled: flagToRaw(r.enabled, p('Rule_Enabled')),
    Combinator: optionalEnumToRaw(r.combinator, p('Combinator')),
    Cooldown_Period: r.cooldownMs,
    Description: r.description,
    Max_Fires_Per_Job: optionalToRaw(r.maxFiresPerJob, p('Max_Fires_Per_Job')),
    Priority: r.priority,
  };
  const attrs = writeFields(values, RULE_FIELDS, r.extra, path);
  const children: Record<string, ExtensionTree> = {};
  for (const c of uniqueSorted(r.conditions, path, 'Condition_Id', 'condition')) {
    children[c.label] = packCondition(c, path);
  }
  return { attrs, children, datasets: {} };
}

function packTriggers(t: TriggerConfig): ExtensionTree {
  const values: Record<string, RawValue> = {
    Triggers_Enabled: flagToRaw(t.enabled, `${TRIGGER}/@Triggers_Enabled`),
    Trigger_Stop_Ceiling_Layers: optionalToRaw(t.stopCeilingLayers, `${TRIGGER}/@Trigger_Stop_Ceiling_Layers`),
  };
  const attrs = writeFields(values, TRIGGER_FIELDS, t.extra, TRIGGER);
  const children: Record<string, ExtensionTree> = {};
  for (const r of uniqueSorted(t.rules, TRIGGER, 'Rule_Id', 'rule')) children[r.label] = packRule(r);
  return { attrs, children, datasets: {} };
}

/** Packs to schema 1.2: every attribute is written with its exact dtype; rules and conditions are sorted by id. */
export function packV1(config: OpcuaConfig): ExtensionTree {
  const attrs = writeFields(
    { [SCHEMA_VERSION_ATTR]: CURRENT_SCHEMA_VERSION },
    [{ name: SCHEMA_VERSION_ATTR, dtype: 'utf8', strict: true }],
    config.extra as ExtraAttrs,
    '',
  );
  return {
    attrs,
    children: { [CLIENT]: packClient(config.client), [PIPE]: packPipe(config.pipe), [TRIGGER]: packTriggers(config.triggers) },
    datasets: {},
  };
}
