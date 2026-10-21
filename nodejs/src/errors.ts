export type ExtensionErrorCode = 'Validation' | 'UnsupportedSchemaVersion' | 'LossyDowngrade';

export class ExtensionError extends Error {
  readonly code: ExtensionErrorCode;
  /** Slash-joined location of the offending node; attributes are prefixed with `@`. */
  readonly path: string;

  constructor(code: ExtensionErrorCode, path: string, message: string) {
    super(message);
    this.name = 'ExtensionError';
    this.code = code;
    this.path = path;
  }
}

export type Result<T, E> = { ok: true; value: T } | { ok: false; error: E };

export function joinPath(path: string, name: string): string {
  return path ? `${path}/${name}` : name;
}

export function validation(path: string, message: string): ExtensionError {
  return new ExtensionError('Validation', path, message);
}

/** Runs `fn`, turning a thrown {@link ExtensionError} into an error result; other errors propagate. */
export function attempt<T>(fn: () => T): Result<T, ExtensionError> {
  try {
    return { ok: true, value: fn() };
  } catch (e) {
    if (e instanceof ExtensionError) return { ok: false, error: e };
    throw e;
  }
}
