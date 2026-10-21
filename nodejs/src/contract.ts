// Structural copy of core's extension contract (types only; core is never imported).

export type ExtensionDtype =
  | 'utf8'
  | 'i8'
  | 'u8'
  | 'i16'
  | 'u16'
  | 'i32'
  | 'u32'
  | 'i64'
  | 'u64'
  | 'f32'
  | 'f64';

export type ExtensionScalar = string | number | bigint;

/** i64/u64 are `bigint`; every other numeric dtype is `number`. */
export interface ExtensionAttr {
  dtype: ExtensionDtype;
  value: ExtensionScalar | ExtensionScalar[];
}

export type ExtensionNumericArray =
  | Int8Array
  | Uint8Array
  | Int16Array
  | Uint16Array
  | Int32Array
  | Uint32Array
  | BigInt64Array
  | BigUint64Array
  | Float32Array
  | Float64Array;

/** Unused by OPC UA; kept so the tree type matches core's. */
export interface ExtensionDataset {
  dtype: Exclude<ExtensionDtype, 'utf8'>;
  shape: number[];
  data: ExtensionNumericArray;
  attrs: Record<string, ExtensionAttr>;
}

export interface ExtensionTree {
  attrs: Record<string, ExtensionAttr>;
  children: Record<string, ExtensionTree>;
  datasets: Record<string, ExtensionDataset>;
}

export const REQUIRES_CONTRACT = '^0.1';
export const EXTENSION_NAME = 'TM_OPCUA';

export function emptyTree(): ExtensionTree {
  return { attrs: {}, children: {}, datasets: {} };
}
