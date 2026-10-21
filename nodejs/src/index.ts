import { unpackLegacy } from './adapters/legacy.js';
import { packV1, unpackV1 } from './adapters/v1.js';
import { type ExtensionTree } from './contract.js';
import { attempt, ExtensionError, validation, type Result } from './errors.js';
import type { OpcuaConfig } from './model.js';
import { CURRENT_SCHEMA_VERSION, SCHEMA_VERSION_ATTR } from './schema-v1.js';

export { EXTENSION_NAME, REQUIRES_CONTRACT, emptyTree } from './contract.js';
export { CURRENT_SCHEMA_VERSION } from './schema-v1.js';
export type {
  ExtensionAttr,
  ExtensionDataset,
  ExtensionDtype,
  ExtensionScalar,
  ExtensionTree,
} from './contract.js';
export { ExtensionError } from './errors.js';
export type { ExtensionErrorCode, Result } from './errors.js';
export { suggestedDefaults } from './defaults.js';
export type { SuggestedDefaults } from './defaults.js';
export type * from './model.js';

/** Schema versions `pack` can write. The legacy layout is read-only and not a version. */
export function supportedSchemaVersions(): string[] {
  return [CURRENT_SCHEMA_VERSION];
}

/** The root version attribute as stored; `'legacy'` when it is absent. */
export function peekSchemaVersion(tree: ExtensionTree): string | 'legacy' {
  const a = tree.attrs[SCHEMA_VERSION_ATTR];
  if (a === undefined) return 'legacy';
  return Array.isArray(a.value) ? a.value.map(String).join(',') : String(a.value);
}

function majorOf(version: string, path: string): number {
  const m = /^(\d+)\.(\d+)$/.exec(version);
  if (!m) throw validation(path, `invalid schema version "${version}"; expected "<major>.<minor>"`);
  return Number(m[1]);
}

export function unpack(tree: ExtensionTree): Result<OpcuaConfig, ExtensionError> {
  return attempt(() => {
    const a = tree.attrs[SCHEMA_VERSION_ATTR];
    if (a === undefined) return unpackLegacy(tree);
    const path = `@${SCHEMA_VERSION_ATTR}`;
    if (a.dtype !== 'utf8' || Array.isArray(a.value)) throw validation(path, 'expected a utf8 scalar');
    const major = majorOf(a.value as string, path);
    if (major !== 1) {
      throw new ExtensionError('UnsupportedSchemaVersion', path, `unsupported schema version "${String(a.value)}"`);
    }
    return unpackV1(tree);
  });
}

/**
 * Packs to `opts.targetSchema` (default: current). Packing the legacy layout is not implemented
 * and returns `LossyDowngrade`.
 */
export function pack(config: OpcuaConfig, opts: { targetSchema?: string } = {}): Result<ExtensionTree, ExtensionError> {
  return attempt(() => {
    const target = opts.targetSchema ?? CURRENT_SCHEMA_VERSION;
    if (target === 'legacy') {
      throw new ExtensionError('LossyDowngrade', '', 'packing to the legacy layout is not implemented');
    }
    if (target !== CURRENT_SCHEMA_VERSION) {
      throw new ExtensionError('UnsupportedSchemaVersion', '', `cannot pack schema version "${target}"; supported: ${CURRENT_SCHEMA_VERSION}`);
    }
    return packV1(config);
  });
}
