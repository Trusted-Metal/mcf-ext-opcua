# mcf-ext-opcua - design

Status: draft for review. Nothing here is implemented yet.

`mcf-ext-opcua` packs and unpacks the OPC UA extension (`TM_OPCUA`) of a machine configuration
file. It is a pure codec: it never opens a file and has no HDF5 dependency. The Machine Config
Library (MCF core) owns all reads and writes and knows nothing about OPC UA.

Sections 1-3 describe the **extension contract**, which lives in MCF core and is shared by every
extension. It is written here first so this repo can be built against it; it moves to core's docs
when core work starts. Sections 4-9 are specific to this repo.

## 1. Why the contract is needed (verified against core `SD-1684`)

- `MachineConfigFileV1_x.open()` parses the whole file into a typed `MachineConfig`
  (`reader.parse`), and `save()` writes a **new** file from that model (`new h5wasm.File(path, "w")`).
- Anything not in the model is lost on a read-modify-save. Unknown groups under `Extensions/` are
  not preserved today; only `extra` attributes on known groups are.
- `getRawGroup` exists in Node only as a read-only reader method. Rust's raw-group methods are on
  the concrete adapter types, which is what couples `rust-opcua-ext` to core.

So core needs one generic, opaque place for extension data, and a rule that it round-trips
unchanged.

## 2. The extension contract

Owned by core, versioned by `EXTENSION_CONTRACT_VERSION` (separate from `File_Version` and from
the package version). `0.x` until ClearBox is ported as the second extension, then `1.0`; within a
major version, changes are additive only.

### 2.1 Tree

```text
ExtensionTree = {
  attrs:    { [name]: Value }            // group attributes
  children: { [name]: ExtensionTree }    // child groups
  datasets: { [name]: Dataset }          // array data (unused by OPC UA; needed by ClearBox)
}
Dataset = { dtype: Dtype, shape: number[], data: flat array in row-major order }
Value   = { dtype: Dtype, value: scalar | array }
```

- `Dtype` is explicit and maps 1:1 to the HDF5 storage type:
  `utf8`, `bool`, `i32`, `u32`, `i64`, `u64`, `f32`, `f64`. Scalar or 1-D array.
  Flags that are stored as `u32` or `f64` must come back with the same dtype they went in.
  Node represents `i64`/`u64` as `bigint`.
- Node names (attribute, child and dataset) must be non-empty and must not contain `/` or NUL.
  Core enforces this on write; it is also what stops a label silently becoming a nested path.
- Order is not significant. HDF5 orders by name, so the canonical form sorts.
- Datasets are in the contract from day one so adding a node kind later is not a breaking change.
  Compression and chunking hints are not included yet; they can be added later as optional fields.

### 2.2 Core behaviour

- `MachineConfig.extensions: { [name]: ExtensionTree }`, keyed by the group name under
  `Extensions/` (`TM_OPCUA`).
- Reading populates it for **every** group under `Extensions/`, known or not.
- Writing replaces each extension's whole subtree with the tree in the map. A rule dropped from
  the tree therefore cannot survive on disk. The prototype had to add orphan pruning by hand; with
  full replace this is a core property.
- An extension absent from the map is not written. Hosts must not drop trees they don't
  understand; that is what preserves private extensions.
- `File_Version` 1.0 stores OPC UA at `OPCUA/`. Core exposes it as `extensions["TM_OPCUA"]` and
  writes it back to the right place, so extensions see one tree shape.
- Core exports `EXTENSION_CONTRACT_VERSION` and a `canonicalDigest(tree)` helper (2.3).
- Core's typed `opcua` and ClearBox models are deprecated while the extensions land and removed at
  its next major release.

### 2.3 Canonical digest

rdt-core owns `configuration_hash` and must include extension content. The digest is SHA-256 over
a canonical encoding, defined once in the contract and identical in Node and Rust:

- children, attributes and datasets sorted by name (byte order of the UTF-8 name);
- each entry encoded as name length + name, dtype tag, shape, then fixed-width little-endian
  values (strings as length-prefixed UTF-8; `-0` and NaN normalised for `f32`/`f64`).

Conformance fixtures store the expected digest for each tree.

### 2.4 Distribution

- Node: types-only subpath export of the core package (`machine-config-library/extension-contract`).
  Extensions declare the interfaces structurally and do not import the core runtime.
- Rust: a workspace crate in the core repo (`mcf-extension-contract`) holding the tree types and
  digest, with no `hdf5` dependency. It is for hosts only; extension crates do not depend on it.
  Like Node, they declare the tree types structurally (same shape as the fixtures' `tree.json`)
  and the host converts at the boundary.
- `assertContractCompatible(coreVersion, requires)` lets the host fail fast on a mismatch.
- Extensions and core release independently. Compatibility is the contract version range an
  extension declares (`REQUIRES_CONTRACT`), checked at runtime by the host, not by a package
  manager. Within a contract major version changes are additive only, so a core release never
  forces an extension release.

## 3. Rules every extension follows

- Two independent version axes: core `File_Version` and the extension's own schema version, read
  from an attribute on the extension's root group. The extension never looks at `File_Version`.
- A schema version is `major.minor`, always 1.x for now. Adapters are keyed by major; a newer
  minor is read by ignoring unknown attributes (kept in `extra`); a newer or unknown major
  returns `UnsupportedSchemaVersion`. Same shape as core's `UnsupportedFileVersion`.
- A group with no schema version attribute is not a version at all: it is the legacy layout
  written by core's typed OPC UA support, handled by a dedicated legacy adapter outside the
  numbering.
- Pack and unpack validate: strict fields, enum ranges, label rules, id uniqueness, dtypes. Errors
  carry a code and a path (`Trigger/Job Start/Condition 1/Value`).
- No silent fallbacks. A fallback exists only when the spec requires it and is documented.
- Pure functions: no I/O, no global state, deterministic output order.

## 4. Public interface of `mcf-ext-opcua`

Same shape in both languages:

```ts
export const EXTENSION_NAME = 'TM_OPCUA';
export const REQUIRES_CONTRACT = '^0.1';           // contract major.minor range

export function supportedSchemaVersions(): string[];
export function peekSchemaVersion(tree: ExtensionTree): string | 'legacy';   // no attribute = 'legacy'
export function unpack(tree: ExtensionTree): Result<OpcuaConfig, ExtensionError>;
export function pack(
  config: OpcuaConfig,
  opts?: { targetSchema?: string },                 // defaults to the current schema
): Result<ExtensionTree, ExtensionError>;
```

`Result`/`ExtensionError` mirror core's `Result`/`CapabilityError` (codes include `Validation`,
`UnsupportedSchemaVersion`, `LossyDowngrade`).

### Domain model (deliberately diverges from the on-disk spec)

The model removes the spec's encoding quirks so consumers never repeat the conversions. Pack
always writes the spec form; unpack always returns the model form. Identical in Node and Rust.

```text
OpcuaConfig { client: ClientConfig, pipe: PipeConfig, triggers: TriggerConfig, extra }
ClientConfig { serverUrl, authMode, machineProfile?, securityMode,
               securityPolicy: Auto | Basic256Sha256 | ECC_*,
               rootNode: AutoDiscover | NodeId(s) | BrowseName(s),
               samplingInterval: ServerNegotiated | Fastest | Ms(n), ..., extra }
PipeConfig   { enabled: bool, configureClient: bool, inboundRateLimitHz?: n, ..., extra }
TriggerConfig { enabled: bool, stopCeilingLayers?: n, rules: Rule[], extra }
Rule { id: n >= 1, label, description, action, combinator?, enabled, priority,
       cooldownMs, maxFiresPerJob?: n, conditions: Condition[], extra }
Condition { id: n >= 1, label, enabled, event, subsystem, component, signal,
            evaluation: String { value, caseSensitivity }
                      | Numeric { threshold: number, operator, epsilon }, extra }
```

| On disk (spec) | Model | Rule |
| --- | --- | --- |
| `u32` flag `0`/`1` | `bool` | any other value is a validation error; always written as `u32` |
| required enum | enum | empty or unknown is a validation error (stricter than the prototype, which accepts `""`) |
| optional enum with default `""` (`combinator`, `machine_profile`) | optional enum | `""` maps to absent and absent is written as `""` |
| `""` that has a real meaning (`security_policy`: SDK auto-select) | explicit variant `Auto` | not optional, because it is a choice, not an unset value |
| `root_node` string | `AutoDiscover` / `NodeId` / `BrowseName` | empty, contains `=`, or neither; matches the spec's three resolution modes |
| `sampling_interval` `-1` / `0` / n | `ServerNegotiated` / `Fastest` / `Ms(n)` | other negative values are a validation error |
| `0` meaning unlimited or unset (`max_fires_per_job`, `inbound_rate_limit`, `trigger_stop_ceiling_layers`) | optional number | absent is written as `0`; a read `inbound_rate_limit` of `-1` is also absent, per the spec note that both mean no cap |
| `rule_id` / `condition_id` `0` | not representable | model ids are `>= 1`; `0` is a validation error on read and write |
| `evaluation_mode` + String-mode and Numeric-mode fields on every condition | tagged union | Numeric fields cannot exist on a String condition; a Numeric condition without an operator or with a non-numeric `value` is a validation error |
| condition `value` string | `value: string` or `threshold: number` | a Numeric threshold is written as the shortest round-trip decimal; fixtures pin the exact text in both languages |
| enum casing | canonical spelling | fields marked case-insensitive in the schema (`machine_profile`, `min_integrity_level`, `user_access_level`) accept any case on read and are written canonical |
| group order | rules and conditions sorted by id | order on disk is not significant |
| unknown attributes | `extra` at each level | preserved on round trip |

Defaults:

- A missing non-strict attribute takes the spec's documented default.
- Profile-dependent real defaults (`sampling_interval` 50 for Aconity, `queue_size_data_change`
  50 for Aconity) are not applied implicitly. An explicit helper such as
  `suggestedDefaults(profile)` can return them for UIs; the library never fills them in.
- Required fields stay required: a missing strict attribute is an error, not a default.

## 5. Adapters

```text
src/
  schema/            generated from schema/*.yaml (see 6)
  adapters/
    legacy.*         attribute absent: Client/, Pipe/, flat Triggers/{label}
    v1.*             "1.x": Client/, Pipe/, Trigger/{rule}/{condition}
  model.*            stable domain model
  facade.*           peek -> dispatch -> unpack / pack
```

- **Legacy (no version attribute)** unpacks flat triggers into rules: one `CaptureStart` rule from `start_value`
  and one `CaptureStop` rule from `stop_value`, each with one String condition. This is the same
  conversion rdt-core's import currently does in `config-import-opcua-legacy.util.ts`, plus the
  legacy enum mapping (`DropOldest` to `DiscardOldest`, `EOS` to `Eos`, hex integrity levels to
  names). It moves here and is removed from rdt-core.
- **Pack to legacy** works only when every rule can be expressed as the flat shape (one
  condition, String mode, start/stop pairs). Otherwise it returns `LossyDowngrade`.
- **Current schema version** is `1.2`, the value the prototype already writes, so no existing
  prototype file needs migrating. Any layout-breaking change after this is `2.0`.
- Pack always writes the schema version attribute.

## 6. Schema source and generation

One YAML file per schema version is the source of truth. Generated: Rust structs and TS types for
the on-disk shape, the validators, and the README field table (currently hand-maintained).

```yaml
schema_version: "1.2"
groups:
  Client:
    fields:
      Auth_Mode:   { dtype: utf8, enum: [Anonymous, UsernamePassword, Certificate], default: Anonymous }
      Machine_Profile: { dtype: utf8, enum: [Aconity, Eos], case_insensitive: true, default: "" }
      Sampling_Interval: { dtype: i32, default: 250, doc: "ms; -1 server-negotiated, 0 as fast as possible" }
  Pipe:
    fields:
      Pipe_Name: { dtype: utf8, pattern: '^\\\\\.\\pipe\\.+$', default: '\\.\pipe\opc_ua_client_pipe' }
      Pipe_Enabled: { dtype: u32, flag: true, default: 0 }
  Trigger:
    fields:
      Triggers_Enabled: { dtype: u32, flag: true, strict: true }
    collection:                       # child groups keyed by a label field
      key: Rule_Label
      unique: [Rule_Id]
      fields:
        Rule_Id: { dtype: u32, strict: true, nonzero: true }
        Action:  { dtype: utf8, strict: true, enum: [CaptureStart, CaptureStop] }
      collection:
        key: Condition_Label
        unique_within_parent: [Condition_Id]
        fields: { ... }
```

Format features needed: dtype, `enum` with optional case-insensitivity, `default`, `strict`
(missing is an error), `nonzero`, `pattern`, `flag`, `min`/`max`, and keyed collections where the
key field is also the child group name (label rules apply to it).

The scalar mapping between disk and model is declared in the same schema file with a `model:`
annotation and generated, so Node and Rust cannot differ:

```yaml
      Pipe_Enabled:        { dtype: u32, flag: true, default: 0, model: { name: enabled } }
      Inbound_Rate_Limit:  { dtype: i32, default: 100, model: { name: inboundRateLimitHz, zero_is_absent: true, also_absent: [-1] } }
      Security_Policy:     { dtype: utf8, enum: [Basic256Sha256, ...], empty_is: Auto }
      Combinator:          { dtype: utf8, enum: [And, Or], empty_is_absent: true }
      Min_Integrity_Level: { dtype: utf8, enum: [...], case_insensitive: true }
```

Generated: flag conversion, zero/empty sentinels, enum casing, defaults, validation. Hand-written
and checked against the schema by the fixtures: the structural parts (`rootNode`,
`samplingInterval`, the `evaluation` union, flattening of collections into lists).
Generating only Node and Rust keeps the generator small; the same schema file is what any later
language would be generated from.

## 7. Testing

- **Fixtures:** `fixtures/<schema>/<case>/{tree.json, model.json, digest.txt}` and
  `fixtures/errors/<case>/{tree.json, expect.json}` for failures (missing strict field, duplicate
  rule label, `/` in a label, bad enum, wrong dtype, `rule_id` 0, unknown major version).
- **Per language:** `unpack(tree) == model`, `pack(model) == tree` (canonical compare), error
  fixtures return the expected code and path, digest equals the stored value.
- **Cross-language:** both suites read the same fixture directory, so Node and Rust cannot drift.
  Capture's hand-written reader can run the same fixtures.
- **Legacy fixtures:** extracted once from core's `fixtures/reference_config_opcua.h5` (rc.7/rc.8)
  by a script that reads with core and dumps the tree. Real legacy data is the regression guard
  for the legacy adapter.
- **Core integration (dev only, optional CI job):** write `pack` output through a real core build,
  re-read the `.h5`, and `unpack` it; also check core preserves a second, unknown extension.
  Core is not a runtime dependency.
- **Compatibility:** CI runs the integration job against the latest few core tags.

## 8. Repository and release

```text
mcf-ext-opcua/
  schema/            YAML per schema version
  fixtures/
  nodejs/            package `mcf-ext-opcua`
  rust/              crate `mcf-ext-opcua`
  tools/             generator, legacy-fixture extractor
  DESIGN.md
```

- One lockstep version for both languages, tagged `vX.Y.Z`, released with semantic-release as core
  does (`rc` pre-releases).
- Package version and schema version are separate. New adapter for a new schema version: minor
  release. Dropping the ability to read an old schema: major release. A required contract major
  bump: major release.
- Distribution: Node release-asset tarball (how rdt-core consumes core today); Rust via git tag.
- No dependency on core in either language, and none on `mcf-extension-contract`. Node has no
  runtime dependencies; Rust depends only on `serde`. Core owns reads and writes;
  this library only parses extension data and prepares it for core to write.

## 9. Migration sequence

1. **Core:** contract (tree, dtypes, datasets), `extensions` on `MachineConfig`, read all /
   write full-replace, legacy `OPCUA/` mapping, digest, preservation tests. Deprecate typed
   `opcua`.
2. **This repo:** seeded from `rust-opcua-ext` on core's `feat/v1_2` with its git history kept
   (subtree split), then: schema format and generator, domain model, legacy and current adapters,
   fixtures, Node and Rust codecs.
3. **rdt-core:** the export mapper packs with this library and passes the tree to core; import
   unpacks from `extensions["TM_OPCUA"]`; remove `config-import-opcua-legacy.util.ts`; include
   the canonical digest in `configuration_hash`. Rule export starts working at this step.
4. **ClearBox:** `mcf-ext-clearbox` on the same contract; contract goes `1.0`.
5. **Core major release:** remove typed OPC UA and ClearBox models.

rdt-core stays on `machine-config-library` rc.7 until steps 1-4 are done.

## 10. Decisions and open questions

Decided:

- Schema versions stay 1.x; the rules layout is `1.2`; no attribute means legacy.
- `i64`/`u64` are in the contract (`bigint` in Node).
- Dataset compression and chunking hints are added later.
- The prototype's git history is carried into this repo.
- The domain model diverges from the on-disk spec as described in section 4.

Open:

- Nothing outstanding for the design. Next step is the core contract (section 9, step 1).
