import type { ExtensionAttr } from './contract.js';

export type AuthMode = 'Anonymous' | 'UsernamePassword' | 'Certificate';
export type MachineProfile = 'Aconity' | 'Eos';
export type SecurityMode = 'None' | 'Sign' | 'SignAndEncrypt';
export type SecurityPolicy =
  | 'Auto'
  | 'Basic256Sha256'
  | 'ECC_nistP256'
  | 'ECC_nistP384'
  | 'ECC_brainpoolP256r1'
  | 'ECC_brainpoolP384r1';
export type QueuePolicy = 'DiscardOldest' | 'KeepOldest';
export type MinIntegrityLevel = 'Untrusted' | 'Low' | 'Medium' | 'High' | 'System' | 'ProtectedProcess';
export type UserAccessLevel = 'CurrentUserOnly' | 'CurrentUserAndAdmins' | 'AnyLocalUser' | 'Unrestricted';
export type RuleAction = 'CaptureStart' | 'CaptureStop';
export type Combinator = 'And' | 'Or';
export type CaseSensitivity = 'Exact' | 'Contains' | 'ExactCaseSensitive' | 'ContainsCaseSensitive';
export type ComparisonOperator = 'Equal' | 'GreaterThan' | 'LessThan' | 'GreaterThanOrEqual' | 'LessThanOrEqual';

/** Attributes the codec does not know about, kept with their dtype and written back unchanged. */
export type ExtraAttrs = Record<string, ExtensionAttr>;

export type RootNode =
  | { kind: 'AutoDiscover' }
  | { kind: 'NodeId'; value: string }
  | { kind: 'BrowseName'; value: string };

export type SamplingInterval =
  | { kind: 'ServerNegotiated' }
  | { kind: 'Fastest' }
  | { kind: 'Ms'; ms: number };

export interface ClientConfig {
  serverUrl: string;
  authMode: AuthMode;
  machineProfile?: MachineProfile;
  securityMode: SecurityMode;
  securityPolicy: SecurityPolicy;
  rootNode: RootNode;
  samplingInterval: SamplingInterval;
  publishInterval: number;
  bfsMaxDepth: number;
  keepAliveCount: number;
  lifetimeCount: number;
  queuePolicy: QueuePolicy;
  queueSizeDataChange: number;
  queueSizeEvents: number;
  reconnectInterval: number;
  sessionTimeout: number;
  syncLoopIntervalInitial: number;
  syncLoopIntervalSettled: number;
  extra: ExtraAttrs;
}

export interface PipeConfig {
  enabled: boolean;
  configureClient: boolean;
  bufferSize: number;
  /** Absent when the on-disk value is 0 or -1 (no cap). */
  inboundRateLimitHz?: number;
  maxInboundMessageSize: number;
  minIntegrityLevel: MinIntegrityLevel;
  pipeName: string;
  userAccessLevel: UserAccessLevel;
  extra: ExtraAttrs;
}

export type Evaluation =
  | { mode: 'String'; value: string; caseSensitivity: CaseSensitivity }
  | { mode: 'Numeric'; threshold: number; operator: ComparisonOperator; epsilon: number };

export interface Condition {
  /** >= 1, unique within the rule. */
  id: number;
  label: string;
  enabled: boolean;
  event: string;
  subsystem: string;
  component: string;
  signal: string;
  evaluation: Evaluation;
  extra: ExtraAttrs;
}

export interface Rule {
  /** >= 1, unique within the config. */
  id: number;
  label: string;
  description: string;
  action: RuleAction;
  combinator?: Combinator;
  enabled: boolean;
  priority: number;
  cooldownMs: number;
  /** Absent when the on-disk value is 0 (unlimited). */
  maxFiresPerJob?: number;
  conditions: Condition[];
  extra: ExtraAttrs;
}

export interface TriggerConfig {
  enabled: boolean;
  /** Absent when the on-disk value is 0. */
  stopCeilingLayers?: number;
  rules: Rule[];
  extra: ExtraAttrs;
}

export interface OpcuaConfig {
  client: ClientConfig;
  pipe: PipeConfig;
  triggers: TriggerConfig;
  /** Unknown root-group attributes (the schema version attribute is not included). */
  extra: ExtraAttrs;
  /** Only set when unpacked from the legacy layout: legacy triggers sides that could not become rules. */
  warnings?: string[];
}
