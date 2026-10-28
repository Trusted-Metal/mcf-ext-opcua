# TM_OPCUA configuration reference

Reference for the OPC UA extension (`TM_OPCUA`) of a machine configuration file, as read and
written by this repo's Rust codec on `feat/capture-compatibility`. It covers the on-disk layout,
how the reader treats missing, blank and unrecognised values, the checks run after reading, and
every field's type, range, default and meaning. The codec only parses and validates: what a
consumer does with the values (capture's trigger engine, client-proj's session) is described in
[CONSUMERS.md](./CONSUMERS.md).

> Preliminary (2026-10-27): reflects the current Rust code. To be finalised after the Node port.

See also:

- [TRIGGER_CONFIG_DESIGN.md](./TRIGGER_CONFIG_DESIGN.md): the full annotated tree from
  `Extensions/` down, and what the trigger schema can express.
- [CONSUMERS.md](./CONSUMERS.md): how capture, client-proj, MCL core and rdt-core use this config.
- [REVIEW_NOTES.md](./REVIEW_NOTES.md): what changed in this pass, decisions and open questions.
- [WORKING_PLAN.md](./WORKING_PLAN.md): remaining work, in order.
- [../DESIGN.md](../DESIGN.md): the extension contract and the original codec design.

Scope note: the Rust codec is current. The Node codec has not received this pass yet, so where
this document and Node disagree, this document describes the Rust behaviour Node will match.

## Design intents

1. **Strict structure, lenient values.** Writers emit every field. Readers reject a missing field
   only where it changes structure, security or matching; everything else falls back. No fallback
   is silent: every default and normalisation is reported as a warning.
2. **Full range, separate from preferred defaults.** Every option a consumer supports is accepted.
   Internal preferences live in the defaults and in per-deployment overrides, never in narrowing
   the accepted values. Weak-but-valid choices are reported (Advisory), not rejected.
3. **Strictest defaults for security.** A blank security field reads as its strictest value.
   Where no value is safe to assume (`Auth_Mode`), a blank is an error.
4. **Contradictions fail; oddities warn.** A combination that can never work (security mode vs.
   policy) is an error on read and on pack. Legal but unlikely values are reported as Suspicious.
5. **Accept what consumers accept.** Spellings client-proj and older tools already understand
   are read as the canonical value, with a warning, so files and pipe messages agree.

## Conventions

- **Names and values are PascalCase on disk.** Attribute names use `Pascal_Snake` (`Server_URL`);
  enum values use PascalCase (`SignAndEncrypt`) except policies, which keep their OPC UA short
  names (`ECC_nistP256`). `pack` always writes the canonical spelling.
- **Reading is case-insensitive** for every enum value, including `User_Access_Level`. A
  casing-only difference reads as the canonical value with a `Normalized` "(casing)" warning.
  Consumers matching raw strings normalise to lowercase before comparing.
- **Attribute names** are matched exactly first, then ignoring case and separators
  (`cooldown_period`, `CooldownPeriod`), with a warning.
- **Flags** are `u32` `0` / `1`. **Sentinels** are explicit: `0` = none / unlimited where a field
  says so; `-1` only where documented (`Sampling_Interval`, `Inbound_Rate_Limit`).
- **Priority:** lower value wins, `0` is the highest precedence.

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

The complete tree with every attribute is in
[TRIGGER_CONFIG_DESIGN.md](./TRIGGER_CONFIG_DESIGN.md#full-reference-tree). MCL core owns all HDF5
I/O and hands the codec an opaque tree. Saving replaces the whole `TM_OPCUA` subtree, so a rule
removed from the model cannot linger on disk.

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

### Missing attribute

| Tier | Fields | Result |
| --- | --- | --- |
| Required | Security / connection: `Server_URL`, `Auth_Mode`, `Security_Mode`, `Security_Policy`, `Pipe_Enabled`, `Pipe_Name`, `User_Access_Level`, `Min_Integrity_Level`, `Triggers_Enabled`. Structural: rule {`Rule_Id` / `Rule_Label` / `Action` / `Rule_Enabled` / `Priority`}, condition {`Condition_Id` / `Condition_Label` / `Condition_Enabled` / `Evaluation_Mode` / `Signal` / `Value`} | Error |
| Required by context | The comparison field the condition's mode uses (`Case_Sensitivity` for String, `Comparison_Operator` for Numeric); `Combinator` when a rule has more than one condition | Error |
| Warned | Everything else: Client and Pipe tuning fields, `Machine_Profile`, `Root_Node`, `Trigger_Stop_Ceiling_Layers`, the optional rule and condition fields, the mode-unused comparison field | Reads as the default, `Defaulted` warning |

A field newer than the file's declared minor reads as its default silently. A required field
that is missing but has a near-miss spelling present names it: `missing required attribute
(found Securty_Mode)`.

### Blank value (`""`)

| Meaning | Fields | Result |
| --- | --- | --- |
| A value in its own right | `Security_Policy` (Auto), `Root_Node` (auto-discover), `Machine_Profile` (none), `Combinator` (none), `Signal`, `Value`, `Description`, `Event` / `Subsystem` / `Component` | Accepted, no warning (checks may still report it) |
| Use the default | `Security_Mode`, `User_Access_Level`, `Min_Integrity_Level` (strictest), `Queue_Policy`, `Evaluation_Mode`, the mode-used comparison field | Default, `Defaulted` warning |
| Not used by this mode | The comparison field the condition's mode ignores | Accepted, no warning |
| No safe default | `Auth_Mode`, `Server_URL`, `Pipe_Name`, `Action`, labels, `@Opcua_Schema_Version` | Error (`must not be blank`) |

### Spellings and casing

The reader maps spellings consumers already accept to the canonical value, with a `Normalized`
warning: `Auth_Mode` `username` / `password`; `Security_Policy` short names and URIs (`NistP384`,
`http://opcfoundation.org/UA/SecurityPolicy#ECC_nistP384`, `_` and case ignored); `Queue_Policy`
`DropOldest`; hex integrity RIDs (`0x2000`). Casing-only differences read as canonical with a
"(casing)" warning.

### Attribute names

- **Variants** of a known name (case or separators differ) are read as that field, with an
  `UnknownName` warning `attribute name read as <Name>`.
- **Duplicates:** when both the canonical name and a variant exist, the canonical one is used and
  the variant is kept and reported as a duplicate spelling.
- **Unknown names** are kept (they round-trip) and reported, with `did you mean <Name>?` when
  exactly one known field is closest within the edit budget. Distance is optimal string alignment
  (an adjacent transposition counts as one edit) over names with case and separators removed.
  Budget: up to 8 characters 1 edit, up to 16 characters 2, longer 3. Ties are not suggested. A
  near miss is never applied as the field.
- In a newer-minor file, unknown names are expected and not reported.

### Dtypes

Integer fields accept any integral dtype (`i8` through `u64`) and `f64` fields accept `f32`, with
a `Normalized` warning `dtype <x> read as <y>`; the value's range is still checked. `pack` writes
the canonical dtype from the tables below.

### Warnings

Warnings are returned on `OpcuaConfig.warnings` as `Warning { kind, path, message }` (displayed as
`<path>: <message>`). For 1.x files the field is absent when there is nothing to report; legacy
files always carry the list (possibly empty) to mark the file as converted. Warnings describe how
the file was read and what the checks found; they are not part of the configuration.

| Kind | Meaning |
| --- | --- |
| `Defaulted` | A missing or blank field took its default |
| `Normalized` | A value, casing or dtype was read as its canonical form |
| `UnknownName` | An attribute name is unknown, a variant, or a duplicate spelling |
| `Conversion` | Legacy layout conversion (legacy `"None"` policy, skipped trigger sides) |
| `Advisory` | Valid but weaker than the preferred posture |
| `Suspicious` | Valid but very likely a mistake |

## Checks

Run on the unpacked model by both readers. `coherence` also runs in `pack`, so an incoherent model
cannot be written.

### Errors (coherence)

| `Security_Mode` | `Security_Policy` | Result |
| --- | --- | --- |
| any | blank (Auto) | OK: the SDK picks a policy matching the mode |
| `None` | `None` | OK (Advisory below) |
| `None` | any real policy | Error at `Client/@Security_Policy` |
| `Sign` / `SignAndEncrypt` | `None` | Error at `Client/@Security_Policy` |
| `Sign` / `SignAndEncrypt` | any real policy | OK |

### Advisory

`Security_Mode None`; `User_Access_Level` `AnyLocalUser` or `Unrestricted`; `Min_Integrity_Level`
`Untrusted` or `Low`; `Max_Inbound_Message_Size 0`; triggers enabled with an enabled Start rule
but no enabled Stop rule and no `Trigger_Stop_Ceiling_Layers`; a blank `Combinator` on a rule
with several conditions.

### Suspicious

`0` for `Reconnect_Interval`, `Sync_Loop_Interval_Initial` / `_Settled`, `Session_Timeout`,
`Keep_Alive_Count`, `Lifetime_Count`, `BFS_Max_Depth` or `Buffer_Size`; `Lifetime_Count` below
3 x `Keep_Alive_Count`; a `Server_URL` without an OPC UA scheme (`opc.tcp://`, `opc.https://`,
`opc.wss://`, `https://`); a `Pipe_Name` over 256 characters; an enabled rule whose conditions are
all disabled; an empty `Signal`, or an empty String `Value`, on an enabled condition.

## Labels and ids

- Rule and condition labels are HDF5 group names: non-empty, no `/`, no NUL. The `Rule_Label` /
  `Condition_Label` attribute must equal its group name.
- Rule labels are unique across the file; condition labels are unique within their rule.
- `Rule_Id` and `Condition_Id` are `>= 1`. Rule ids are unique across the file; condition ids are
  unique within their rule. Ids, not labels, are the stable identity consumers key on, so a rule
  can be renamed without changing its identity.
- The model returns rules and conditions sorted by id. On-disk order is not significant.

## Field reference

Columns:

- **Attribute** on disk, **dtype** written by `pack`, **Model** field on the unpacked Rust model.
- **Missing** / **Blank**: what the reader does (see [How values are read](#how-values-are-read)).
  "default" means the Default column's value with a `Defaulted` warning.
- **Range**: every accepted value.
- **Default**: what a missing or blank field reads as. For required fields it is the value
  `suggested_defaults()` offers for a new config (UIs); the codec never fills those in. `-` means
  no default exists.
- **Description**: meaning, sentinels and related checks.

### Client (`ClientConfig`): OPC UA session and subscription

| Attribute | dtype | Model | Missing | Blank | Range | Default | Description |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Auth_Mode` | utf8 | `auth_mode` | error | error | `[Anonymous / UsernamePassword / Certificate]` | `Certificate` (suggested) | User identity presented when the session is activated. `username` / `password` read as `UsernamePassword` |
| `BFS_Max_Depth` | u32 | `bfs_max_depth` | default | - | `0`-`u32::MAX` | `16` | Browse depth for address-space discovery. `0` is Suspicious |
| `Keep_Alive_Count` | u32 | `keep_alive_count` | default | - | `0`-`u32::MAX` | `240` | Publish cycles without data before the server sends a keep-alive. `0` is Suspicious |
| `Lifetime_Count` | u32 | `lifetime_count` | default | - | `0`-`u32::MAX` | `2400` | Publish cycles without a client request before the subscription expires. Below 3 x `Keep_Alive_Count` is Suspicious (servers revise it upward) |
| `Machine_Profile` | utf8 | `machine_profile` (`Option`) | default | none | `[Aconity / Eos]` or blank | blank (none) | Selects browse strategy and profile behaviour; consumers treat none as Aconity. Aconity changes the suggested `Sampling_Interval` and `Queue_Size_Data_Change` |
| `Publish_Interval` | u32 | `publish_interval` | default | - | `0`-`u32::MAX` ms | `250` | Interval between notification batches from the server. No sentinel values |
| `Queue_Policy` | utf8 | `queue_policy` | default | default | `[DiscardOldest / KeepOldest]` | `DiscardOldest` | What a full monitored-item queue drops. Legacy `DropOldest` reads as `DiscardOldest` |
| `Queue_Size_Data_Change` | u32 | `queue_size_data_change` | default | - | `0`-`u32::MAX` | `10` (suggested `50` for Aconity) | Monitored-item queue size for data changes |
| `Queue_Size_Events` | u32 | `queue_size_events` | default | - | `0`-`u32::MAX` | `10` | Monitored-item queue size for events |
| `Reconnect_Interval` | u32 | `reconnect_interval` | default | - | `0`-`u32::MAX` ms | `1000` | Delay between reconnect attempts. `0` is Suspicious |
| `Root_Node` | utf8 | `root_node` (`RootNode`) | default | auto-discover | NodeId, BrowseName or blank | blank (`AutoDiscover`) | Where browsing starts. Contains `=`: a NodeId (`i=85`, `ns=2;s=X`, `nsu=<uri>;s=X`); otherwise a BrowseName under ObjectsFolder |
| `Sampling_Interval` | i32 | `sampling_interval` (`SamplingInterval`) | default | - | `-1`, `0`, `1`-`i32::MAX` ms | `250` (suggested `50` for Aconity) | `-1` (`ServerNegotiated`): inherit the subscription's publishing interval; `0` (`Fastest`): the fastest rate the server offers; `n`: ms. Other negatives are errors |
| `Security_Mode` | utf8 | `security_mode` | error | `SignAndEncrypt` (strictest) | `[None / Sign / SignAndEncrypt]` | `SignAndEncrypt` | Message security for the secure channel. `None` is Advisory and must pair with policy `None` or blank (see Checks) |
| `Security_Policy` | utf8 | `security_policy` | error | Auto | `[None / Basic256Sha256 / Aes128_Sha256_RsaOaep / Aes256_Sha256_RsaPss / ECC_nistP256 / ECC_nistP384 / ECC_brainpoolP256r1 / ECC_brainpoolP384r1]` or blank | blank (Auto) | Algorithm suite. Blank lets the SDK choose one matching the mode. `None` is the no-security policy. Short names and policy URIs are accepted |
| `Server_URL` | utf8 | `server_url` | error | error | text | - | Endpoint URL; convention `opc.tcp://HOST:PORT/CONFIG_TEMPLATE_ID/SERVER_INSTANCE_NAME`. A non-OPC UA scheme is Suspicious |
| `Session_Timeout` | u32 | `session_timeout` | default | - | `0`-`u32::MAX` ms | `60000` | Session lifetime without activity. `0` is Suspicious |
| `Sync_Loop_Interval_Initial` | u32 | `sync_loop_interval_initial` | default | - | `0`-`u32::MAX` ms | `1000` | Interval between subscription syncs before settling. `0` is Suspicious |
| `Sync_Loop_Interval_Settled` | u32 | `sync_loop_interval_settled` | default | - | `0`-`u32::MAX` ms | `30000` | Interval between subscription syncs after settling. `0` is Suspicious |

### Pipe (`PipeConfig`): capture <-> client transport

| Attribute | dtype | Model | Missing | Blank | Range | Default | Description |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Buffer_Size` | u32 | `buffer_size` | default | - | `0`-`u32::MAX` bytes | `65536` | Pipe buffer size. `0` is Suspicious |
| `Configure_Client` | u32 flag | `configure_client` (`bool`) | default | - | `[0 / 1]` | `0` | `1`: capture pushes the Client group to client-proj as a `client_config` message |
| `Inbound_Rate_Limit` | i32 | `inbound_rate_limit_hz` (`Option`) | default | - | `-1`, `0`, `1`-`i32::MAX` Hz | `100` | Maximum processed messages per second. `0` or `-1`: no cap (read as absent, written as `0`). Other negatives are errors |
| `Max_Inbound_Message_Size` | u32 | `max_inbound_message_size` | default | - | `0`-`u32::MAX` bytes | `4096` | Larger messages are dropped. `0`: no limit (Advisory) |
| `Min_Integrity_Level` | utf8 | `min_integrity_level` | error | `ProtectedProcess` (strictest) | `[Untrusted / Low / Medium / High / System / ProtectedProcess]` | `ProtectedProcess` | Minimum Windows integrity level of a connecting process. Hex RIDs `0x0000`-`0x5000` are accepted. `Untrusted` / `Low` are Advisory. See [Deployment overrides](#deployment-overrides) |
| `Pipe_Enabled` | u32 flag | `enabled` (`bool`) | error | - | `[0 / 1]` | `0` (suggested) | Starts capture's pipe listener |
| `Pipe_Name` | utf8 | `pipe_name` | error | error | `\\.\pipe\` followed by a name | `\\.\pipe\opc_ua_client_pipe` (suggested) | Windows named pipe; Linux consumers derive their socket path from it. Over 256 characters is Suspicious |
| `User_Access_Level` | utf8 | `user_access_level` | error | `CurrentUserOnly` (strictest) | `[CurrentUserOnly / CurrentUserAndAdmins / AnyLocalUser / Unrestricted]` | `CurrentUserOnly` | Which local accounts may connect (pipe DACL). `AnyLocalUser` / `Unrestricted` are Advisory. See [Deployment overrides](#deployment-overrides) |

### Trigger (`TriggerConfig`): global switches

| Attribute | dtype | Model | Missing | Blank | Range | Default | Description |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Triggers_Enabled` | u32 flag | `enabled` (`bool`) | error | - | `[0 / 1]` | - | Global switch, independent of each rule's own flag |
| `Trigger_Stop_Ceiling_Layers` | u32 | `stop_ceiling_layers` (`Option`) | default | - | `0`-`u32::MAX` | `0` | Layer ceiling for a trigger-started capture with no other layer count. `0`: none (read as absent). No compiled fallback by design |

### Rule (`Rule`): one group per rule under `Trigger/`

| Attribute | dtype | Model | Missing | Blank | Range | Default | Description |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Rule_Id` | u32 | `id` | error | - | `1`-`u32::MAX`, unique in the file | - | Stable identity; consumers key state on it |
| `Rule_Label` | utf8 | `label` | error | error | text, no `/` or NUL; unique | - | Must equal the group name |
| `Action` | utf8 | `action` | error | error | `[CaptureStart / CaptureStop]` | - | What the rule does when it fires |
| `Rule_Enabled` | u32 flag | `enabled` (`bool`) | error | - | `[0 / 1]` | - | Per-rule switch |
| `Priority` | u32 | `priority` | error | - | `0`-`u32::MAX` | - | Lower value wins a Start/Stop conflict; `0` is the highest precedence. Required so no rule takes top precedence by omission |
| `Combinator` | utf8 | `combinator` (`Option`) | default; error with more than one condition | none (Advisory with more than one condition) | `[And / Or]` or blank | blank (none) | How conditions are joined |
| `Cooldown_Period` | u32 | `cooldown_ms` | default | - | `0`-`u32::MAX` ms | `0` | Minimum time between consecutive fires |
| `Description` | utf8 | `description` | default | kept | text | blank | Free text |
| `Max_Fires_Per_Job` | u32 | `max_fires_per_job` (`Option`) | default | - | `0`-`u32::MAX` | `0` | `0`: unlimited (read as absent) |

### Condition (`Condition`): one group per condition under its rule

| Attribute | dtype | Model | Missing | Blank | Range | Default | Description |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Condition_Id` | u32 | `id` | error | - | `1`-`u32::MAX`, unique in the rule | - | Stable identity within the rule |
| `Condition_Label` | utf8 | `label` | error | error | text, no `/` or NUL; unique in the rule | - | Must equal the group name |
| `Condition_Enabled` | u32 flag | `enabled` (`bool`) | error | - | `[0 / 1]` | - | Per-condition switch |
| `Evaluation_Mode` | utf8 | selects `evaluation` | error | default | `[String / Numeric]` | `String` | How `Value` is compared |
| `Signal` | utf8 | `signal` | error | kept | text | - | Signal name the condition watches. Empty on an enabled condition is Suspicious (consumers skip it) |
| `Value` | utf8 | `evaluation.value` / `.threshold` | error | kept | String: text. Numeric: a finite number | - | Compared text, or numeric threshold. Empty String value on an enabled condition is Suspicious |
| `Event` / `Subsystem` / `Component` | utf8 | `event` / `subsystem` / `component` | default | kept | text | blank | Narrow which signal source matches; empty = any |
| `Case_Sensitivity` | utf8 | `evaluation.case_sensitivity` (String) | String: error. Numeric: default | String: default. Numeric: ignored | `[Exact / Contains / ExactCaseSensitive / ContainsCaseSensitive]` | `Exact` | String-mode match rule |
| `Comparison_Operator` | utf8 | `evaluation.operator` (Numeric) | Numeric: error. String: default | Numeric: default. String: ignored | `[Equal / NotEqual / GreaterThan / LessThan / GreaterThanOrEqual / LessThanOrEqual]` | `Equal` | Numeric-mode comparison |
| `Epsilon` | f64 | `evaluation.epsilon` (Numeric) | default | - | `>= 0`, finite | `0.0` | Numeric tolerance; `0` = the consumer's default tolerance. Unused in String mode |

The model's `evaluation` is a tagged union, so a String condition cannot carry an operator and a
Numeric condition cannot carry a case rule. Writers still emit all three comparison fields on
every condition.

## Legacy layout

Files without `@Opcua_Schema_Version` use the layout core's old typed OPC UA support wrote:
`Client/` and `Pipe/` with the same attribute names, and `Triggers/{label}/` flat triggers each
holding one `Signal` with a `Start_Value` and a `Stop_Value`.

- Missing Client/Pipe attributes take the documented legacy defaults (the old layout made most of
  them optional). `Server_URL`, `Auth_Mode`, `Security_Mode`, `Security_Policy` and
  `Sampling_Interval` are still required. Missing pipe security fields read as the strictest
  values.
- Each flat trigger becomes up to two rules, `<label> - Start` (`CaptureStart`) and
  `<label> - Stop` (`CaptureStop`), each with one String condition, `Combinator And`, priority
  `0`. A side with no signal or value is skipped with a `Conversion` warning. Rule ids are
  assigned sequentially in label order.
- Legacy vocabulary is translated silently: `DropOldest`, hex integrity levels, casing.
- `Security_Policy "None"` reads as the no-security policy, with a `Conversion` warning. Earlier
  codec versions read it as Auto. Files whose `"None"` meant "no preference" should use `""`.
- The coherence check applies to legacy files too: `Security_Mode None` with a real policy, which
  older readers loaded, is now an error.
- Old files are being upgraded to 1.2 (unpack the legacy tree, pack to 1.2, write with core).

## Deployment overrides

The Pipe security fields default to the strictest posture (`CurrentUserOnly`, `ProtectedProcess`).
`ProtectedProcess` admits no ordinary process, so the default fails closed until a deployment
chooses its level. Deployments where capture and client-proj run under different identities
(containers with different UIDs, some service setups) set both explicitly in that machine's
config, typically `AnyLocalUser` and `Medium`. Those are per-deployment overrides, not defaults:
every supported value is accepted, the weaker ones are reported as Advisory, and the strict
defaults fail visibly and trace straight to these two fields.
