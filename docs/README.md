# TM_OPCUA configuration reference

Reference for the OPC UA extension (`TM_OPCUA`) of a machine configuration file, as read and
written by this repo's Rust codec on `feat/capture-compatibility`. It covers the on-disk layout,
how the reader treats missing, blank and unrecognised values, and every field's type, values and
meaning. The codec only parses and validates: what a consumer does with the values (capture's
trigger engine, client-proj's session) is described in [CONSUMERS.md](./CONSUMERS.md).

See also:

- [TRIGGER_CONFIG_DESIGN.md](./TRIGGER_CONFIG_DESIGN.md): what the trigger schema can express, and
  how it improves on the legacy flat-trigger layout.
- [CONSUMERS.md](./CONSUMERS.md): how capture, client-proj, MCL core and rdt-core use this config,
  and what each must handle.
- [REVIEW_NOTES.md](./REVIEW_NOTES.md): what changed in this pass, decisions, open questions and
  test coverage. Start there when reviewing.
- [../DESIGN.md](../DESIGN.md): the extension contract and the original codec design.

Scope note: the Rust codec is current. The Node codec has not received this pass yet (see
REVIEW_NOTES), so where this document and Node disagree, this document describes the Rust
behaviour that Node will be brought in line with.

## Layout

```text
Extensions/TM_OPCUA/            File_Version 1.1 (core exposes File_Version 1.0's OPCUA/ the same way)
  @Opcua_Schema_Version          "1.2" - selects the reader; absent = legacy layout
  Client/                        flat attributes: OPC UA session and subscription
  Pipe/                          flat attributes: capture <-> client IPC transport and its security
  Trigger/                       flat attributes: global trigger switches
    {Rule_Label}/                one group per rule
      {Condition_Label}/         one group per condition
```

MCL core owns all HDF5 I/O and hands the codec an opaque tree. Saving replaces the whole `TM_OPCUA` subtree, so a rule removed from the model cannot linger on disk.

## Version selection

| `@Opcua_Schema_Version` | Reader | Notes |
| --- | --- | --- |
| absent | legacy adapter | Layout written by core's old typed OPC UA support (`Triggers/{label}` flat triggers). Converted to the current model; see [Legacy layout](#legacy-layout). |
| `1.<minor>` | 1.x adapter | Fields introduced after the file's minor are never required; unknown attributes in a newer-minor file are expected and not warned. |
| other major | none | `UnsupportedSchemaVersion` error. |
| not `<major>.<minor>` | none | `Validation` error. |

`pack` always writes schema `1.2` and every field. Packing to the legacy layout is not supported
(`LossyDowngrade`).

## How values are read

Writers emit every field. Readers keep the structure strict and the values lenient.

**Missing attribute:**

| Tier | Fields | Result |
| --- | --- | --- |
| Required | All Client and Pipe fields, both Trigger fields, rule: {`Rule_Id` / `Rule_Label` / `Action` / `Rule_Enabled` / `Priority`}, condition: {`Condition_Id` / `Condition_Label` / `Condition_Enabled` / `Evaluation_Mode` / `Signal` / `Value`} | Error |
| Required by context | The comparison field the condition's mode uses (`Case_Sensitivity` for String, `Comparison_Operator` for Numeric); `Combinator` when a rule has more than one condition | Error |
| Warned | Everything else (`Combinator` on single-condition rules, `Cooldown_Period`, `Description`, `Max_Fires_Per_Job`, `Event`, `Subsystem`, `Component`, the mode-unused comparison field, `Epsilon`) | Reads as the default, with a warning |

**Blank value (`""`):** always accepted. It means one of three things, per field:

- *A value in its own right*: `Security_Policy` (Auto), `Root_Node` (auto-discover),
  `Machine_Profile` (none), `Combinator` (none). No warning.
- *Use the default*: security and connection enums read as their strictest or consumer default,
  with a warning. `Action` is the exception: no safe default exists, so a blank is an error.
- *Not used by this mode*: the comparison field the condition's mode ignores. No warning.

**Spellings:** consumers already accept some alternative spellings (client-proj's identity and
security-policy resolvers, older tools' hex integrity levels). The reader maps them to the
canonical value and warns. Casing-only differences are normalised silently, except
`User_Access_Level`, which is deliberately case-sensitive.

**Unknown attribute names:** kept unchanged (they round-trip) and warned, with a "did you mean"
suggestion when the name is within two edits of a known field. In a file declaring a newer minor
than the reader knows, unknown names are expected and not warned.

**Dtypes:** the 1.x reader requires each attribute's exact dtype (table below). The legacy reader
accepts any integral numeric dtype for integer fields.

**Warnings** are returned on `OpcuaConfig.warnings` as `"<path>: <message>"` lines. For 1.x files
the field is absent when there is nothing to report; legacy files always carry the list (possibly
empty). Warnings describe how the file was read; they are not part of the configuration.

## Labels and ids

- Rule and condition labels are HDF5 group names: non-empty, no `/`, no NUL. The `Rule_Label` /
  `Condition_Label` attribute must equal its group name.
- Rule labels are unique across the file; condition labels are unique within their rule.
- `Rule_Id` and `Condition_Id` are `>= 1`. Rule ids are unique across the file; condition ids are
  unique within their rule. Ids, not labels, are the stable identity consumers key on, so a rule
  can be renamed without changing its identity.
- The model returns rules and conditions sorted by id. On-disk order is not significant.

## Field reference

Columns: **Attribute** (on disk), **dtype**, **Model** (Rust field on the unpacked model),
**Missing** / **Blank** (see above), **Values**, and **Suggested** default. Suggested defaults
come from `suggested_defaults(profile)` and exist for UIs building a new config. The codec never
fills them in.

### Client (`ClientConfig`): OPC UA session and subscription

| Attribute | dtype | Model | Missing | Blank | Values | Suggested |
| --- | --- | --- | --- | --- | --- | --- |
| `Auth_Mode` | utf8 | `auth_mode` | error | `Anonymous` + warning | `[Anonymous / UsernamePassword / Certificate]`; any casing; `username` / `password` read as `UsernamePassword` + warning | `Anonymous` |
| `BFS_Max_Depth` | u32 | `bfs_max_depth` | error | - | browse depth for address-space discovery | `16` |
| `Keep_Alive_Count` | u32 | `keep_alive_count` | error | - | publish cycles without data before a keep-alive request | `240` |
| `Lifetime_Count` | u32 | `lifetime_count` | error | - | publish cycles without a request before the subscription expires | `2400` |
| `Machine_Profile` | utf8 | `machine_profile` (`Option`) | error | none | `[Aconity / Eos]`, case-insensitive; selects browse strategy and profile behaviour | none (consumers treat none as Aconity) |
| `Publish_Interval` | u32 | `publish_interval` | error | - | ms between server batches; no sentinel values | `250` |
| `Queue_Policy` | utf8 | `queue_policy` | error | `DiscardOldest` + warning | `[DiscardOldest / KeepOldest]`; legacy `DropOldest` read as `DiscardOldest` + warning | `DiscardOldest` |
| `Queue_Size_Data_Change` | u32 | `queue_size_data_change` | error | - | monitored-item queue size | `10` (`50` for Aconity) |
| `Queue_Size_Events` | u32 | `queue_size_events` | error | - | event queue size | `10` |
| `Reconnect_Interval` | u32 | `reconnect_interval` | error | - | ms between reconnect attempts | `1000` |
| `Root_Node` | utf8 | `root_node` (`RootNode`) | error | `AutoDiscover` | contains `=`: `NodeId` (`i=85`, `ns=2;s=X`, `nsu=<uri>;s=X`); otherwise `BrowseName` under ObjectsFolder; blank: auto-discovery | `AutoDiscover` |
| `Sampling_Interval` | i32 | `sampling_interval` (`SamplingInterval`) | error | - | `-1` = `ServerNegotiated` (inherit the subscription's publishing interval); `0` = `Fastest`; `n > 0` = `Ms(n)`; other negatives: error | `250` (`50` for Aconity) |
| `Security_Mode` | utf8 | `security_mode` | error | `SignAndEncrypt` + warning | `[None / Sign / SignAndEncrypt]` | `None` (see REVIEW_NOTES) |
| `Security_Policy` | utf8 | `security_policy` | error | `Auto` | `[None / Basic256Sha256 / Aes128_Sha256_RsaOaep / Aes256_Sha256_RsaPss / ECC_nistP256 / ECC_nistP384 / ECC_brainpoolP256r1 / ECC_brainpoolP384r1]`; client-proj spellings (`NistP384`, policy URIs, `_` and case ignored) read as canonical + warning; `None` is the no-security policy | `Auto` |
| `Server_URL` | utf8 | `server_url` | error | kept | free text; convention `opc.tcp://HOST:PORT/CONFIG_TEMPLATE_ID/SERVER_INSTANCE_NAME` | - |
| `Session_Timeout` | u32 | `session_timeout` | error | - | ms | `60000` |
| `Sync_Loop_Interval_Initial` | u32 | `sync_loop_interval_initial` | error | - | ms between subscription syncs before settling | `1000` |
| `Sync_Loop_Interval_Settled` | u32 | `sync_loop_interval_settled` | error | - | ms between subscription syncs after settling | `30000` |

### Pipe (`PipeConfig`): capture <-> client transport

| Attribute | dtype | Model | Missing | Blank | Values | Suggested |
| --- | --- | --- | --- | --- | --- | --- |
| `Buffer_Size` | u32 | `buffer_size` | error | - | pipe buffer, bytes | `65536` |
| `Configure_Client` | u32 flag | `configure_client` (`bool`) | error | - | `[0 / 1]`; `1`: capture pushes the Client group to client-proj | `0` |
| `Inbound_Rate_Limit` | i32 | `inbound_rate_limit_hz` (`Option`) | error | - | `0` or `-1`: no cap (read as absent, written as `0`); `n > 0`: Hz; other negatives: error | `100` |
| `Max_Inbound_Message_Size` | u32 | `max_inbound_message_size` | error | - | bytes; `0` = no size limit (a supported option) | `4096` |
| `Min_Integrity_Level` | utf8 | `min_integrity_level` | error | `ProtectedProcess` + warning | `[Untrusted / Low / Medium / High / System / ProtectedProcess]`, case-insensitive; hex RIDs `0x0000`-`0x5000` read as names + warning | `ProtectedProcess` |
| `Pipe_Enabled` | u32 flag | `enabled` (`bool`) | error | - | `[0 / 1]` | `0` |
| `Pipe_Name` | utf8 | `pipe_name` | error | error | must be `\\.\pipe\` followed by a name; Linux consumers derive their socket path from it | `\\.\pipe\opc_ua_client_pipe` |
| `User_Access_Level` | utf8 | `user_access_level` | error | `CurrentUserOnly` + warning | `[CurrentUserOnly / CurrentUserAndAdmins / AnyLocalUser / Unrestricted]`; casing is strict (deliberate) | `CurrentUserOnly` |

### Trigger (`TriggerConfig`): global switches

| Attribute | dtype | Model | Missing | Values |
| --- | --- | --- | --- | --- |
| `Triggers_Enabled` | u32 flag | `enabled` (`bool`) | error | `[0 / 1]`; global kill-switch independent of each rule's flag |
| `Trigger_Stop_Ceiling_Layers` | u32 | `stop_ceiling_layers` (`Option`) | error | `0`: none (read as absent); `n > 0`: layer ceiling used when a trigger-started capture has no other layer count. No compiled fallback by design. |

### Rule (`Rule`): one group per rule under `Trigger/`

| Attribute | dtype | Model | Missing | Blank | Values |
| --- | --- | --- | --- | --- | --- |
| `Rule_Id` | u32 | `id` | error | - | `>= 1`, unique across the file |
| `Rule_Label` | utf8 | `label` | error | error | equals the group name; unique; no `/` or NUL |
| `Action` | utf8 | `action` | error | error | `[CaptureStart / CaptureStop]` |
| `Rule_Enabled` | u32 flag | `enabled` (`bool`) | error | - | `[0 / 1]` |
| `Priority` | u32 | `priority` | error | - | lower value wins a conflict; `0` is the highest precedence |
| `Combinator` | utf8 | `combinator` (`Option`) | default `""` + warning; error with more than one condition | none; with more than one condition also warns (consumers then evaluate only one condition) | `[And / Or]` |
| `Cooldown_Period` | u32 | `cooldown_ms` | `0` + warning | - | ms between consecutive fires |
| `Description` | utf8 | `description` | `""` + warning | kept | free text |
| `Max_Fires_Per_Job` | u32 | `max_fires_per_job` (`Option`) | `0` + warning | - | `0`: unlimited (read as absent) |

### Condition (`Condition`): one group per condition under its rule

| Attribute | dtype | Model | Missing | Blank | Values |
| --- | --- | --- | --- | --- | --- |
| `Condition_Id` | u32 | `id` | error | - | `>= 1`, unique within the rule |
| `Condition_Label` | utf8 | `label` | error | error | equals the group name; unique within the rule |
| `Condition_Enabled` | u32 flag | `enabled` (`bool`) | error | - | `[0 / 1]` |
| `Evaluation_Mode` | utf8 | selects `evaluation` | error | `String` + warning | `[String / Numeric]` |
| `Signal` | utf8 | `signal` | error | kept | signal name the condition watches (consumers skip a condition with an empty signal) |
| `Value` | utf8 | `evaluation.value` / `.threshold` | error | kept | String: compared text. Numeric: a finite number literal, otherwise an error |
| `Event` / `Subsystem` / `Component` | utf8 | `event` / `subsystem` / `component` | `""` + warning | kept | narrow which signal source matches; empty = any |
| `Case_Sensitivity` | utf8 | `evaluation.case_sensitivity` (String) | String: error. Numeric: `"Exact"` + warning | String: `Exact` + warning. Numeric: ignored | `[Exact / Contains / ExactCaseSensitive / ContainsCaseSensitive]` |
| `Comparison_Operator` | utf8 | `evaluation.operator` (Numeric) | Numeric: error. String: `"Equal"` + warning | Numeric: `Equal` + warning. String: ignored | `[Equal / NotEqual / GreaterThan / LessThan / GreaterThanOrEqual / LessThanOrEqual]` |
| `Epsilon` | f64 | `evaluation.epsilon` (Numeric) | `0.0` + warning | - | `>= 0`; `0` = the consumer's default tolerance; unused in String mode |

The model's `evaluation` is a tagged union, so a String condition cannot carry an operator and a
Numeric condition cannot carry a case rule. Writers still emit all three comparison fields on
every condition; see REVIEW_NOTES for what `pack` currently writes in the unused ones.

## Legacy layout

Files without `@Opcua_Schema_Version` use the layout core's old typed OPC UA support wrote:
`Client/` and `Pipe/` with the same attribute names, and `Triggers/{label}/` flat triggers each
holding one `Signal` with a `Start_Value` and a `Stop_Value`.

- Missing Client/Pipe attributes take the documented legacy defaults (the old layout made most of
  them optional). `Server_URL`, `Auth_Mode`, `Security_Mode`, `Security_Policy` and
  `Sampling_Interval` are still required.
- Each flat trigger becomes up to two rules, `<label> - Start` (`CaptureStart`) and
  `<label> - Stop` (`CaptureStop`), each with one String condition, `Combinator And`, priority
  `0`. A side with no signal or value is skipped with a warning. Rule ids are assigned
  sequentially in label order.
- Legacy vocabulary is translated silently: `DropOldest`, hex integrity levels, casing.
- `Security_Policy "None"` reads as the no-security policy, with a warning. Earlier codec versions
  read it as Auto. Files whose `"None"` meant "no preference" should be updated to `""`.
- Old files are being upgraded to 1.2 (unpack the legacy tree, pack to 1.2, write with core).

## Deployment overrides

The Pipe security fields default to the strictest posture (`CurrentUserOnly`, `ProtectedProcess`).
Deployments where capture and client-proj run under different identities (containers with
different UIDs, some service setups) must set both explicitly in that machine's config, typically
`AnyLocalUser` and `Medium`. Those are per-deployment overrides, not defaults: every supported
value is accepted, and the strict defaults fail visibly and trace straight to these two fields.
