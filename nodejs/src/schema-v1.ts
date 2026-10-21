import type { FieldSpec } from './fields.js';

export const SCHEMA_VERSION_ATTR = 'Opcua_Schema_Version';
export const CURRENT_SCHEMA_VERSION = '1.2';

export const AUTH_MODES = ['Anonymous', 'UsernamePassword', 'Certificate'] as const;
export const SECURITY_MODES = ['None', 'Sign', 'SignAndEncrypt'] as const;
// '' = let the SDK auto-select (model: Auto).
export const SECURITY_POLICIES = [
  '',
  'Basic256Sha256',
  'ECC_nistP256',
  'ECC_nistP384',
  'ECC_brainpoolP256r1',
  'ECC_brainpoolP384r1',
] as const;
export const QUEUE_POLICIES = ['DiscardOldest', 'KeepOldest'] as const;
export const MACHINE_PROFILES = ['', 'Aconity', 'Eos'] as const;
export const MIN_INTEGRITY_LEVELS = ['Untrusted', 'Low', 'Medium', 'High', 'System', 'ProtectedProcess'] as const;
export const USER_ACCESS_LEVELS = ['CurrentUserOnly', 'CurrentUserAndAdmins', 'AnyLocalUser', 'Unrestricted'] as const;
export const RULE_ACTIONS = ['CaptureStart', 'CaptureStop'] as const;
export const COMBINATORS = ['', 'And', 'Or'] as const;
export const EVALUATION_MODES = ['String', 'Numeric'] as const;
export const CASE_SENSITIVITIES = ['Exact', 'Contains', 'ExactCaseSensitive', 'ContainsCaseSensitive'] as const;
export const COMPARISON_OPERATORS = [
  'Equal',
  'GreaterThan',
  'LessThan',
  'GreaterThanOrEqual',
  'LessThanOrEqual',
] as const;

export const PIPE_NAME_PREFIX = '\\\\.\\pipe\\';

const req = (name: string, dtype: FieldSpec['dtype'], extra: Partial<FieldSpec> = {}): FieldSpec => ({
  name,
  dtype,
  strict: true,
  ...extra,
});
const opt = (name: string, dtype: FieldSpec['dtype'], def: string | number, extra: Partial<FieldSpec> = {}): FieldSpec => ({
  name,
  dtype,
  strict: false,
  default: def,
  ...extra,
});

export const CLIENT_FIELDS: readonly FieldSpec[] = [
  req('Auth_Mode', 'utf8', { enum: AUTH_MODES }),
  req('BFS_Max_Depth', 'u32'),
  req('Keep_Alive_Count', 'u32'),
  req('Lifetime_Count', 'u32'),
  req('Machine_Profile', 'utf8', { enum: MACHINE_PROFILES, caseInsensitive: true }),
  req('Publish_Interval', 'u32'),
  req('Queue_Policy', 'utf8', { enum: QUEUE_POLICIES }),
  req('Queue_Size_Data_Change', 'u32'),
  req('Queue_Size_Events', 'u32'),
  req('Reconnect_Interval', 'u32'),
  req('Root_Node', 'utf8'),
  req('Sampling_Interval', 'i32'),
  req('Security_Mode', 'utf8', { enum: SECURITY_MODES }),
  req('Security_Policy', 'utf8', { enum: SECURITY_POLICIES }),
  req('Server_URL', 'utf8'),
  req('Session_Timeout', 'u32'),
  req('Sync_Loop_Interval_Initial', 'u32'),
  req('Sync_Loop_Interval_Settled', 'u32'),
];

export const PIPE_FIELDS: readonly FieldSpec[] = [
  req('Buffer_Size', 'u32'),
  req('Configure_Client', 'u32', { flag: true }),
  req('Inbound_Rate_Limit', 'i32'),
  req('Max_Inbound_Message_Size', 'u32'),
  req('Min_Integrity_Level', 'utf8', { enum: MIN_INTEGRITY_LEVELS, caseInsensitive: true }),
  req('Pipe_Enabled', 'u32', { flag: true }),
  req('Pipe_Name', 'utf8', { startsWith: PIPE_NAME_PREFIX }),
  req('User_Access_Level', 'utf8', { enum: USER_ACCESS_LEVELS }),
];

export const TRIGGER_FIELDS: readonly FieldSpec[] = [
  req('Triggers_Enabled', 'u32', { flag: true }),
  req('Trigger_Stop_Ceiling_Layers', 'u32'),
];

export const RULE_FIELDS: readonly FieldSpec[] = [
  req('Rule_Id', 'u32', { nonzero: true }),
  req('Rule_Label', 'utf8'),
  req('Action', 'utf8', { enum: RULE_ACTIONS }),
  req('Rule_Enabled', 'u32', { flag: true }),
  opt('Combinator', 'utf8', '', { enum: COMBINATORS }),
  opt('Cooldown_Period', 'u32', 0),
  opt('Description', 'utf8', ''),
  opt('Max_Fires_Per_Job', 'u32', 0),
  opt('Priority', 'u32', 0),
];

export const CONDITION_FIELDS: readonly FieldSpec[] = [
  req('Condition_Id', 'u32', { nonzero: true }),
  req('Condition_Label', 'utf8'),
  req('Condition_Enabled', 'u32', { flag: true }),
  req('Signal', 'utf8'),
  req('Value', 'utf8'),
  opt('Event', 'utf8', ''),
  opt('Subsystem', 'utf8', ''),
  opt('Component', 'utf8', ''),
  opt('Evaluation_Mode', 'utf8', 'String', { enum: EVALUATION_MODES }),
  opt('Case_Sensitivity', 'utf8', 'Exact', { enum: CASE_SENSITIVITIES }),
  opt('Comparison_Operator', 'utf8', 'Equal', { enum: COMPARISON_OPERATORS }),
  opt('Epsilon', 'f64', 0),
];
