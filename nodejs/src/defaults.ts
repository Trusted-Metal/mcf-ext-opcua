import type { ClientConfig, MachineProfile, PipeConfig } from './model.js';

export type SuggestedClientDefaults = Omit<ClientConfig, 'serverUrl' | 'machineProfile' | 'extra'>;
export type SuggestedPipeDefaults = Omit<PipeConfig, 'extra'>;

export interface SuggestedDefaults {
  client: SuggestedClientDefaults;
  pipe: SuggestedPipeDefaults;
}

/**
 * Documented defaults for building a new config (e.g. in a UI). `pack` never applies them.
 * Aconity differs from the base values in `samplingInterval` and `queueSizeDataChange`.
 * `authMode`, `securityMode`, `securityPolicy` and `rootNode` are the neutral choices
 * (Anonymous, None, Auto, AutoDiscover).
 */
export function suggestedDefaults(profile?: MachineProfile): SuggestedDefaults {
  const aconity = profile === 'Aconity';
  return {
    client: {
      authMode: 'Anonymous',
      securityMode: 'None',
      securityPolicy: 'Auto',
      rootNode: { kind: 'AutoDiscover' },
      samplingInterval: { kind: 'Ms', ms: aconity ? 50 : 250 },
      publishInterval: 250,
      bfsMaxDepth: 16,
      keepAliveCount: 240,
      lifetimeCount: 2400,
      queuePolicy: 'DiscardOldest',
      queueSizeDataChange: aconity ? 50 : 10,
      queueSizeEvents: 10,
      reconnectInterval: 1000,
      sessionTimeout: 60000,
      syncLoopIntervalInitial: 1000,
      syncLoopIntervalSettled: 30000,
    },
    pipe: {
      enabled: false,
      configureClient: false,
      bufferSize: 65536,
      inboundRateLimitHz: 100,
      maxInboundMessageSize: 4096,
      minIntegrityLevel: 'ProtectedProcess',
      pipeName: '\\\\.\\pipe\\opc_ua_client_pipe',
      userAccessLevel: 'CurrentUserOnly',
    },
  };
}
