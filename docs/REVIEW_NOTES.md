# Review notes: capture-compatibility pass

State of `feat/capture-compatibility` (Rust codec) as of 2026-10-27 (preliminary), for review
before the Node port, docs and merge. Field-level detail is in [README.md](./README.md); consumer
impact is in [CONSUMERS.md](./CONSUMERS.md); remaining work, in order, is in
[WORKING_PLAN.md](./WORKING_PLAN.md).

## What changed from 1.0.0-rc.2

### Read rules

- **Tiered presence.** Missing security/connection fields, structural trigger fields and the
  mode-used comparison field are errors. Every other missing field (Client and Pipe tuning,
  `Machine_Profile`, `Root_Node`, `Trigger_Stop_Ceiling_Layers`, optional rule/condition fields)
  reads as its default with a `Defaulted` warning. `Evaluation_Mode` and `Priority` became
  required; `Combinator` is required on rules with more than one condition.
- **Blanks.** A blank is a value in its own right (Auto policy, auto-discover root, no profile, no
  combinator, free text), a request for the default (strictest for security fields, with a
  warning), or the normal form of a mode-unused field. Where no value is safe to assume, a blank
  is an error: `Auth_Mode`, `Server_URL`, `Pipe_Name`, `Action`, labels.
- **Required fields are version-scoped.** A field introduced after the file's declared minor is
  never required.
- **Spellings consumers accept are normalized, with a warning**: `Auth_Mode` aliases,
  `Security_Policy` short names and URIs, `Queue_Policy` `DropOldest`, hex integrity levels.
- **Casing.** Every enum, including `User_Access_Level`, reads in any casing with a "(casing)"
  warning. (Reverses the earlier strict `User_Access_Level` decision.)
- **Attribute names.** Case/separator variants are read as the field with a warning; a variant
  next to the canonical name is reported as a duplicate; unknown names get a did-you-mean when one
  field is uniquely closest within a length-scaled edit budget; a missing required field names a
  near miss (`found X`). Unknown names in newer-minor files are not reported.
- **Dtypes.** Integer fields accept any integral dtype and `f64` accepts `f32` on 1.x reads, with a
  warning; ranges are still checked.
- **Negative `Epsilon` is rejected.**

### Checks (new, `src/checks.rs`)

- **Coherence errors**, on read and on pack: `Security_Mode None` with a real policy;
  `Sign` / `SignAndEncrypt` with policy `None`.
- **Advisory**: weaker-than-preferred choices (mode `None`, open pipe access, low integrity, no
  message size limit, Start rules with no way to stop, blank `Combinator` with several conditions).
- **Suspicious**: legal but likely mistakes (zero intervals and sizes, short lifetime, non-OPC UA
  URL, long pipe name, rules that can never fire, empty signal or value).

### Values (full range)

- `Security_Policy` adds `None`, `Aes128_Sha256_RsaOaep` and `Aes256_Sha256_RsaPss`.
- `Comparison_Operator` adds `NotEqual`.
- `Max_Inbound_Message_Size = 0` is documented as "no size limit" (Advisory).

### Legacy adapter

- `Security_Policy "None"` reads as the no-security policy (`Conversion` warning) instead of Auto.
- The coherence check applies: mode `None` with a real policy now fails.
- The vocabulary mapping (hex levels, `DropOldest`) uses the shared normalizers, silently.
- Unknown attributes are reported.

### API

- `OpcuaConfig.warnings: Option<Vec<Warning>>` with `Warning { kind, path, message }` and
  `WarningKind [Defaulted / Normalized / UnknownName / Conversion / Advisory / Suspicious]`.
  Filled for 1.x files too (absent when empty); legacy files always carry the list.
- `RootNode::to_spec`, `SamplingInterval::to_spec` and `SecurityPolicy::to_spec` return the on-disk
  encoding.
- `ClientConfig::to_message() -> ClientConfigMessage`: the `client_config` pipe message in the
  on-disk encoding, with a `schema` field; JSON schema in `schema/messages/`, shared fixtures in
  `fixtures/messages/client_config/`.
- `suggested_defaults()` suggests `SignAndEncrypt`, policy Auto and `Certificate` auth.

## Design decisions behind the changes (2026-10-22 to 2026-10-27)

1. **Strict structure, lenient values.** Writers emit every field; readers reject a missing field
   only where it changes structure, security or matching, and report every default and
   normalization. This replaces DESIGN.md section 3's "no silent fallbacks" with "no *silent*
   fallbacks". The tiered rule was reconsidered on 10-27 and kept.
2. **Full range, separate from preferred defaults.** Every option a component supports is
   accepted. Preferences live in defaults or per-deployment overrides; weak choices are Advisory.
3. **Strictest defaults for security fields.** Blank `Security_Mode`, `User_Access_Level`,
   `Min_Integrity_Level` read as `SignAndEncrypt`, `CurrentUserOnly`, `ProtectedProcess`. Blank
   `Auth_Mode` is an error because no identity is safe to assume.
4. **Contradictions fail, oddities warn.** Only pairings that can never connect are errors.
5. **Case-insensitive enums everywhere**, with a warning (10-27; supersedes the 10-22 strict
   `User_Access_Level` exception).
6. **Priority: lower value wins, 0 is highest**, and Priority is required.
7. **Legacy `"None"` is the no-security policy.** Old files are being upgraded.
8. **The extension owns the `client_config` message**, so the pipe contract is versioned with the
   schema rather than reconstructed in capture.

## Where this departs from DESIGN.md

| DESIGN.md | Now | Section to update |
| --- | --- | --- |
| Status "draft, nothing implemented"; README says package `0.x` | Implemented; package `1.0.0-rc.2` | header, README table |
| Section 3: "No silent fallbacks"; a missing strict attribute is an error, others take the spec default | Tiered presence; every default and normalization is warned | 3, 4 |
| Section 4 table: "required enum: empty or unknown is a validation error" | Blank reads as the default (strictest for security), is a value, or (Auth_Mode) an error; unknown is still an error | 4 |
| Section 4: `user_access_level` case-insensitive | Matches again: every enum is case-insensitive, with a warning | 4 (wording) |
| Section 4: `security_policy` list | Adds `None` and the two Aes policies | 4 |
| No cross-field validation | `checks.rs`: coherence errors plus Advisory / Suspicious | new section |
| Section 5: legacy maps `"None"` to Auto | Legacy `"None"` is the no-security policy | 5 |
| Section 6: YAML schema and generator | Not built; field tables are hand-written in `schema_v1.rs` and `schema-v1.ts`, guarded by the shared fixtures | 6 |
| Section 2.4 vs. section 8 on depending on `mcf-extension-contract` | The codec declares its own tree types; hosts convert through the contract JSON form | 2.4 / 8 (pick one) |

## Open questions and pending work

Ordered work is in [WORKING_PLAN.md](./WORKING_PLAN.md). Codec items still open:

- [x] `pack` writes `"Exact"` / `"Equal"` / `0.0` in mode-unused comparison fields; the convention
  is blank.
- [ ] Rename `SamplingInterval::ServerNegotiated` (`-1` = inherit the subscription's publishing
  interval).
- [ ] Datastream profiles as fixtures (`fixtures/datastream/<profile>/`).
- [x] `suggested_defaults` security posture (now `SignAndEncrypt` / Auto / `Certificate`).
- [x] Duplicate `.one_of(SECURITY_MODES)` on `Security_Mode`.
- [x] Integral dtype widening on 1.x reads.
- [x] `schema_v1.rs`: duplicate `Max_Inbound_Message_Size` entry removed; `Publish_Interval`
  default corrected to `250`.
- [x] Shared default constants (`defaults::base`) used by `schema_v1.rs` and `suggested_defaults`,
  with a unique-names unit test and a missing/blank-reads-as-suggested parity test.

## Test coverage

**Rust suite**: 114 tests, all passing, clippy and fmt clean (2026-10-27): unit (1), checks
(14), codec_fixtures (5), contract (8), errors (25), facade (7), fixtures (2), legacy (16),
messages (1), roundtrip (9), tiers (26). `tests/common/tree.rs` holds the typed tree helpers and warning filters
shared by `tiers.rs` and `checks.rs`.

Missing Client/Pipe tuning fields and blank security fields reading as the suggested defaults are
covered by `facade.rs` (parity test). Not yet covered: blank `Auth_Mode` and plain-text blanks as
errors, missing `Trigger_Stop_Ceiling_Layers` / `Configure_Client`.

**End to end with real files** (2026-10-26, before the tiered-defaults change; re-run pending):
eight profiles written by `aconity-datastream`'s h5 writer onto a File_Version 1.1 base, read by
core's CLI, unpacked by the codec and loaded in capture (desktop, `opcua-win`). `demo` has since
been made coherent (mode `None`, policy `""`) and regenerated.

| Profile | Result (10-26) |
| --- | --- |
| `tm`, `ursa`, `demo`, `full_range`, `pending_full_range` | Loaded, schema 1.2, expected rules, no warnings |
| `lenient` | Loaded with 7 warnings (3 spellings, 2 strict-default blanks, 2 condition blanks) |
| `invalid` | Rejected at `@Value` (non-numeric threshold); capture disabled OPC UA and loaded the rest |
| `legacy_like` | Read as legacy and rejected at `Trigger`; capture disabled OPC UA and loaded the rest |

Not yet covered end to end: the pipe listener's per-profile security settings, and client-proj
receiving `client_config` from these files.
