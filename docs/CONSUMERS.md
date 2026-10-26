# Consumers of the TM_OPCUA config

The codec stops at a validated, typed model. This document records how each downstream component
reads that model, what it must handle, and where its behaviour differs from what the config alone
suggests. Pipeline for context: machine -> adapter -> OPC UA server -> client-proj (OPC UA client)
-> capture (clearbox-tauri). The machine config is loaded by capture; capture pushes the Client
group to client-proj over the pipe.

| Consumer | Uses | Status |
| --- | --- | --- |
| [MCL core](#mcl-core) | Carries `TM_OPCUA` as an opaque extension tree | Contract shipped (0.2.0-rc.9); no facade getter yet |
| [capture](#capture) | Full model: pipe, client push, trigger engine | Ported on `feat/hybrid-trigger`; load-tested with all datastream profiles |
| [client-proj](#client-proj) | The Client group, received from capture as JSON | No change needed; contract documented below |
| [rdt-core](#rdt-core) | Import/export of machine configs | Not migrated (DESIGN.md section 9, step 3) |
| Node codec | Same model in TypeScript | Not yet at parity with this pass |

## MCL core

- Core reads every group under `Extensions/` into `MachineConfig.extensions` and writes each one
  back by replacing its whole subtree. File_Version 1.0's `OPCUA/` group is exposed as
  `extensions["TM_OPCUA"]` and written back to `OPCUA/`.
- Core's `configuration_hash` includes a canonical digest of each extension tree. Re-saving a file
  after the codec has normalised it (canonical spellings, filled-in defaults) changes the hash.
- The session facade has no extension getter yet. Rust consumers currently re-read the file with
  `MachineConfigReader::open(path)?.parse()?.extensions` (a second parse). A
  `get_extension` / `set_extension` pair on the facade is planned on MCL `feat/capture-compatibility`.
- Core's tree type (`mcf_extension_contract::ExtensionTree`) and the codec's are separate Rust
  types with the same contract JSON form. Rust hosts convert with
  `serde_json::from_value(tree_to_json(&core_tree))`.

## Capture

Capture holds the unpacked model as `LoadedConfig.opcua: Option<OpcuaConfig>`.

### Loading
- No `TM_OPCUA` group: `opcua = None`, pipe and triggers off.
- Invalid `TM_OPCUA` (any unpack error): `opcua = None`, an error log with the code and path, and
  the rest of the machine config (lasers, correction data, build info) still loads. A bad OPC UA
  setting disables OPC UA, not capture.
- Codec warnings are logged one per line as `TM_OPCUA: <path>: <message>`.
- Rules are ordered by priority (lower first), then id. (Decided 2026-10-26; capture's reader sort
  is being switched from its earlier highest-first order, see REVIEW_NOTES.)

### Pipe
| Config | Capture behaviour |
| --- | --- |
| `Pipe_Enabled` | Starts the listener; a reload to a config without an enabled pipe waits instead of recreating it |
| `User_Access_Level` | Pipe DACL (`PipeSecurity`), mapped exhaustively from the enum |
| `Min_Integrity_Level` | Minimum client integrity RID: Untrusted `0x0000` through ProtectedProcess `0x5000` |
| `Inbound_Rate_Limit` | Minimum interval between processed messages; absent = no throttle |
| `Max_Inbound_Message_Size` | Messages larger than this are dropped and logged; `0` = no limit |
| `Pipe_Name` | Windows named pipe; on Linux the UDS socket path is derived from it |

**Client push (`client_config` message)**: when `Pipe_Enabled` and `Configure_Client` are both
set, capture sends the Client group to client-proj, using the on-disk encodings (the contract
client-proj parses), not the model's own JSON form:

| Key | Sent as |
| --- | --- |
| `auth_mode`, `queue_policy`, `security_mode` | canonical string |
| `machine_profile` | canonical string, or `""` when absent (not `null`: client-proj treats `null` as "keep the previous value") |
| `root_node` | `RootNode::to_spec()`: `""`, the NodeId text, or the BrowseName |
| `sampling_interval` | `SamplingInterval::to_spec()`: `-1`, `0` or ms |
| `security_policy` | `SecurityPolicy::to_spec()`: `""` for Auto (never `"Auto"`), otherwise the canonical name |
| numeric fields | as-is |

**Trigger engine**: capture converts each rule into its own engine types. Things to know when
writing rules:

- `Exact` and `Contains` ignore case and trim whitespace; the `...CaseSensitive` variants do not
  ignore case.
- `Epsilon <= 0` uses the engine default tolerance `1e-6`.
- Numeric comparison accepts numeric strings and JSON booleans (`true` = 1) from the signal.
- A rule with no combinator and several enabled conditions evaluates only condition id 1, or the
  lowest-id enabled one. The codec warns about this when loading.
- Cooldowns, fire counts and edge state are keyed by rule id. A fire count re-arms on that rule's
  own falling edge.
- Start/Stop conflicts in one evaluation pass: lower priority value wins; a tie favours Start.
- `Max_Fires_Per_Job` absent and `Trigger_Stop_Ceiling_Layers` absent map to the engine's `0`
  ("unlimited" / "none").
- Runtime overrides from the UI use the same value names and are merged by rule and condition id.
  The override UI also offers `NotEqual`, which files can now express too.

## client-proj

client-proj receives the `client_config` message above and binds each key to a nullable field.

- **Accepted spellings**: `auth_mode` case-insensitive with `username` / `password` aliases;
  `security_policy` by short name, alias or URI (`SecurityPolicyNames.Canonical`);
  `machine_profile` case-insensitive, `""` = Aconity. The codec accepts the same spellings when
  reading a file and normalises them, so files and pipe messages agree.
- **Queue policy**: only `KeepOldest` is checked explicitly; anything else behaves as
  `DiscardOldest`.
- **Sampling interval**: `-1` means the monitored item inherits the subscription's publishing
  interval; `0` is the fastest rate the server offers.
- **Security mode `None`**: valid in the config, but client-proj's endpoint selection currently
  skips no-security endpoints, so a `None` mode cannot connect yet. Under the full-range rule this
  belongs in client-proj's preference order rather than being excluded; tracked as a client-proj
  follow-up.
- **Types**: `root_node` binds as a string and `sampling_interval` as an integer. Sending the
  model's tagged form (`{"kind": ...}`) fails deserialization, which is why capture sends the
  on-disk encodings.

## rdt-core

Not migrated yet. DESIGN.md section 9 step 3: export packs with this codec and passes the tree to
core; import unpacks from `extensions["TM_OPCUA"]`; `config-import-opcua-legacy.util.ts` is
removed because the codec's legacy adapter replaces it; the extension digest feeds
`configuration_hash`.
