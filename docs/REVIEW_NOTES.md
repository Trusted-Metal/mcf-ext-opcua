# Review notes: capture-compatibility pass

State of `feat/capture-compatibility` (Rust codec) as of 2026-10-26, for review before the Node
port, docs and merge. Field-level detail is in [README.md](./README.md); consumer impact is in
[CONSUMERS.md](./CONSUMERS.md).

## What changed from 1.0.0-rc.2

### Read rules

- **Tiered presence.** Missing structural, security/connection and matching fields are errors;
  other missing fields read as their default with a warning (README, How values are read).
  `Evaluation_Mode` and `Priority` became required. The comparison field a condition's mode uses
  is required, and `Combinator` is required on rules with more than one condition.
- **Blanks accepted.** A blank is either a value in its own right (Auto policy, auto-discover root,
  no profile, no combinator), a request for the default (strictest for security fields, with a
  warning), or the normal form of a mode-unused field. Blank `Action` stays an error.
- **Required fields are version-scoped.** A field introduced after the file's declared minor is
  never required, so older-minor files stay valid as the schema grows.
- **Spellings consumers accept are normalised, with a warning**: `Auth_Mode` aliases,
  `Security_Policy` short names and URIs, `Queue_Policy` `DropOldest`, hex integrity levels.
  Casing-only changes stay silent; `User_Access_Level` casing stays strict.
- **Unknown attribute names** are warned (with did-you-mean suggestions) instead of being kept
  silently; unknown names in newer-minor files are not warned.
- **Negative `Epsilon` is rejected.**

### Values (full range)

- `Security_Policy` adds `None`, `Aes128_Sha256_RsaOaep` and `Aes256_Sha256_RsaPss`.
- `Comparison_Operator` adds `NotEqual`.
- `Max_Inbound_Message_Size = 0` is documented as "no size limit".

### Legacy adapter

- `Security_Policy "None"` reads as the no-security policy (with a warning) instead of Auto.
- The vocabulary mapping (hex levels, `DropOldest`) moved into the shared normalisers and is
  translated silently.
- Unknown attributes are warned.

### API

- `OpcuaConfig.warnings` is now filled for 1.x files too (absent when there is nothing to report).
- `RootNode::to_spec`, `SamplingInterval::to_spec` and `SecurityPolicy::to_spec` return the on-disk
  encoding, for consumers that forward config (capture -> client-proj).

## Design decisions behind the changes (all 2026-10-22 to 2026-10-26)

1. **Strict structure, lenient values.** Writers emit every field; readers reject a missing field
   only where it changes structure, security or matching, accept blanks everywhere, and warn
   whenever they default or normalise. This replaces DESIGN.md section 3's blanket "no silent
   fallbacks" with "no *silent* fallbacks": every fallback is reported.
2. **Full range, separate from preferred defaults.** Every option a component supports is
   accepted. Internal preferences (stricter security, tuned performance) live in defaults or in
   per-deployment overrides, never in narrowing the accepted values.
3. **Strictest defaults for security fields.** A blank `Security_Mode`, `User_Access_Level` or
   `Min_Integrity_Level` reads as `SignAndEncrypt`, `CurrentUserOnly` or `ProtectedProcess`.
4. **`User_Access_Level` casing stays strict**, by choice, unlike the other enums.
5. **Priority: lower value wins, 0 is highest.** Priority is required so an omitted value cannot
   take the highest precedence.
6. **Legacy `"None"` is the no-security policy.** Old files are being upgraded, so the earlier
   "None means Auto" reading is not preserved.

## Where this departs from DESIGN.md

| DESIGN.md | Now | Section to update |
| --- | --- | --- |
| Status "draft, nothing implemented"; README says package `0.x` | Implemented; package `1.0.0-rc.2` | header, README table |
| Section 3: "No silent fallbacks"; a missing strict attribute is an error, others take the spec default | Tiered presence; every default and normalisation is warned | 3, 4 |
| Section 4 table: "required enum: empty or unknown is a validation error" | Blank reads as the default (strictest for security fields) with a warning; unknown is still an error | 4 |
| Section 4 table: `user_access_level` listed as case-insensitive | Case-sensitive by decision | 4 |
| Section 4: `security_policy` list | Adds `None` and the two Aes policies | 4 |
| Section 5: legacy maps `"None"` to Auto (implicitly, via the prototype's behaviour) | Legacy `"None"` is the no-security policy | 5 |
| Section 6: YAML schema and generator | Not built; field tables are hand-written in `schema_v1.rs` and `schema-v1.ts`, guarded by the shared fixtures | 6 |
| Section 2.4: extension crates depend on `mcf-extension-contract` (core's EXTENSION_CONTRACT.md) vs. section 8: no dependency | The codec declares its own tree types; hosts convert through the contract JSON form | 2.4 / 8 (pick one) |

## Open questions and pending work

### Codec (Rust)

- [ ] `pack` writes `"Exact"` / `"Equal"` / `0.0` in the mode-unused comparison fields; the agreed
  convention is blank. Changing it regenerates `fixtures/v1_2/rich/tree.json`.
- [ ] Rename `SamplingInterval::ServerNegotiated`. `-1` means "inherit the subscription's publishing
  interval" (client-proj's definition), not "server-negotiated". The variant name is in
  `lenient_read`'s `model.json`.
- [ ] `suggested_defaults` still suggests `Security_Mode None`; the strict-defaults rule suggests
  `SignAndEncrypt`.
- [ ] Duplicate `.one_of(SECURITY_MODES)` on `Security_Mode` in `schema_v1.rs` (harmless; remove).
- [ ] Optional: accept any integral dtype on 1.x reads (the legacy reader already does); default an
  empty label attribute to its group name; make legacy rule ids independent of label order.
- [ ] Add the datastream profiles as fixtures (`fixtures/datastream/<profile>/` with expected
  results) so real writer output is regression-tested.

### Node

- [ ] Port this pass: presence tiers, blank handling, normalisers, new values, 1.x warnings,
  legacy `"None"`, `to_spec`. Bring the shared fixtures back to one set both codecs pass.

### Consumers

- [ ] Capture: switch the reader's rule sort to lower-priority-first; replace the inline
  `client_config` JSON with a typed message struct; surface the load error in the UI; frontend
  and test updates; Linux `pipe-uds` build check.
- [ ] MCL: facade `get_extension` / `set_extension` so hosts drop the second parse.
- [ ] client-proj: offer every security mode and policy and keep preference as an order, rather
  than excluding no-security endpoints (full-range rule).
- [ ] Upgrade remaining legacy files to 1.2, setting `Security_Policy ""` where `"None"` meant "no
  preference".

## Test coverage

**Rust suite**: all passing. `tests/tiers.rs` (new, 18 tests) covers missing versus blank per tier,
mode-dependent fields, strict defaults, normalisation, `User_Access_Level` casing, the new values,
negative epsilon, did-you-mean warnings, newer-minor silence and `to_spec`. Existing suites were
adjusted where the new rules changed expectations:

- `codec_fixtures` and `roundtrip` compare models without `warnings`.
- `lenient_read`'s tree gained `Evaluation_Mode`, `Case_Sensitivity` and a blank `Combinator`.
  These are values the fixture already defaulted, so its model is unchanged.
- `errors` uses an unrecognised `Queue_Policy` instead of a blank one.
- `legacy` expects the no-security reading of `"None"`.

**End to end with real files**: eight profiles written by `aconity-datastream`'s h5 writer onto a
File_Version 1.1 base. Each was read by core's CLI, unpacked by the codec, and loaded in capture
(desktop, `opcua-win`):

| Profile | Result |
| --- | --- |
| `tm`, `ursa`, `demo`, `full_range`, `pending_full_range` | Loaded, schema 1.2, expected rules, no warnings |
| `lenient` | Loaded with 7 warnings (3 spellings, 2 strict-default blanks, 2 condition blanks) |
| `invalid` | Rejected at `@Value` (non-numeric threshold); capture disabled OPC UA and loaded the rest |
| `legacy_like` | Read as legacy and rejected at `Trigger`; capture disabled OPC UA and loaded the rest |

Not yet covered end to end: the pipe listener's per-profile security settings, and client-proj
receiving `client_config` from these files.
