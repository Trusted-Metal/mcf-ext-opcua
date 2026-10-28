# Working plan: finishing the capture-compatibility pass

Working document, not reference documentation. Tracks what remains to finish `mcf-ext-opcua`
`feat/capture-compatibility` and fold it into MCL and capture. Updated 2026-10-27.

Approach: **fold in, don't replace.** Capture `main` already took PR #94 (SD-1740: pins mcf-ext
`1.0.0-rc.2` and MCL `b485b81`, parses once with `parse_with_binary()`, converts to the legacy DTO
in `machine_config/opcua.rs`, CI SSH fixes). This branch keeps #94's structure and brings this
pass's improvements into it.

Branches:

| Repo | Branch | State |
| --- | --- | --- |
| mcf-ext-opcua | `feat/capture-compatibility` | 2 commits ahead of `1.0.0-rc.2` plus uncommitted work |
| Machine_Config_Library | `feat/capture-compatibility` | Same as `main` (`b485b81`, `0.2.0-rc.9`) |
| clearbox-tauri | `feat/hybrid-trigger` | 67 commits behind `main` (includes #94 and `1.0.2-rc.4`); local path dependencies |

## Done this pass

| Item | Notes |
| --- | --- |
| Tiered read rule | Kept: missing tuning fields read as the default with a warning; structural, security/connection and mode-used fields are errors |
| Structured warnings | `Warning { kind, path, message }`, six kinds |
| Checks module | `coherence` (errors, also on pack) and `advise` (Advisory / Suspicious) |
| Name matching | Case/separator variants accepted with a warning; duplicate spellings named; did-you-mean with a length-scaled edit budget |
| Casing | Every enum, including `User_Access_Level`, reads in any casing with a "(casing)" warning |
| Blanks | `Blank::Invalid` enforced for plain text; optional fields accept blank; `Auth_Mode` blank is an error |
| Suggested defaults | `SignAndEncrypt`, policy `Auto`, auth `Certificate` |
| `client_config` message | `ClientConfig::to_message()`, JSON schema, shared fixtures regenerated |
| Last-pass test fixes 1-9 | Legacy `E0382`, fixtures, `without_warnings` shared, `wrong_dtype`, message fixtures |
| Datastream `demo` profile | Now coherent (mode `None`, policy `""`); regenerated |

## Remaining, in recommended order

Docs, tests, CI and the Node port are held until last (see [Later](#later)).

| # | Area | Remaining gap | Fold-in with main / #94 | Size |
| --- | --- | --- | --- | --- |
| 1 | mcf-ext Rust | ~~Duplicate `Max_Inbound_Message_Size`; `Publish_Interval` default~~ Done 10-27 | - | - |
| 2 | mcf-ext Rust | ~~Shared default constants + parity tests~~ Done 10-27; `Inbound_Rate_Limit` still uses a literal `100` | - | S |
| 3 | mcf-ext Rust | ~~`pack` writes `"Exact"` / `"Equal"` / `0.0` in mode-unused comparison fields; the agreed convention is blank (regenerates `rich` fixture)~~ | - | S |
| 4 | mcf-ext Rust | ~~`cargo fmt`, clippy clean, commit~~ Done 10-27 (`feat!` commit, `BREAKING CHANGE` footer) | - | - |
| 5 | Datastream writer | ~~Regenerate the 8 profiles against the tiered schema~~ Done 10-27; the capture load test re-runs in item 13 | - | - |
| 6 | mcf-ext pin | ~~Cut the next rc and re-pin capture to the tag~~ Superseded: a tag only comes from a release on `main`, which runs the Node tests (blocked until L1). Push the branch and pin capture to its commit SHA (`rev = "<sha>"`), as `main` already pins by rev. Cut the rc tag when the branch merges to `main` after the Node port, then re-pin to it | Capture `main` pins `2c6e246` by rev; keep pinning by rev until the tag exists | S |
| 7 | Capture: merge | Merge `main` into `feat/hybrid-trigger`; resolve `machine_config/` (#94's `opcua.rs` vs `types.rs` / `reader.rs`) | Keep #94's single parse and CI SSH fixes; drop the second-parse fallback | M |
| 8 | Capture: model | #94 converts to the old DTO; this branch uses `OpcuaConfig` | Keep `opcua.rs` as the boundary, have it return `OpcuaConfig`; retire the DTO | M |
| 9 | Capture: bad config | #94 fails the whole config load on a malformed OPC UA section; this branch sets `opcua = None`, logs, and loads the rest | Raise with Richard; recommend `None` + error log + UI surface | S (decision) |
| 10 | Capture: warnings | Log by kind: Advisory / Suspicious at warn, the rest at info; UI later | Add to #94's load path | S |
| 11 | Capture: behaviour | Priority ascending (0 highest, tie favours Start); payload via `client.to_message()`; delete temporary helpers | Check #94's `rule_fire_counts.clear()` change against the hybrid-trigger engine | M |
| 12 | Capture: hygiene | `image_patcher` common module; remove test-result XMLs committed in `0fac8ef`; settle the 1-line `reader.rs` change; path dependencies back to git pins before the PR | - | S |
| 13 | Functional test | End to end: datastream files -> capture -> pipe -> client-proj, including per-profile pipe security | - | M |
| 14 | client-proj | Check `schema` on `client_config`; accept the spec encoding (`root_node` string, `sampling_interval` int, `""` = Auto); offer every security mode/policy as a preference order; `IdentityResolver.Resolve("certificate")` silently falls back to username, then anonymous, when no user cert is loaded (fail-open, against the strict-security intent) | - | M |
| 15 | Old config files | Re-save legacy and earlier 1.2 files through `pack` (adds `Priority`, fixes Sign + `"None"` pairings, blank `Auth_Mode`) | - | S |
| 16 | MCL | No change needed for capture (#94 uses `parse_with_binary()`). Facade `get_extension` / `set_extension` only when a writer needs it | Branch stays empty until then | S |

## Later

| # | Area | Work |
| --- | --- | --- |
| L1 | Node | Port the pass: tiers, warnings and kinds, name matching, casing, blanks, checks, widening, `to_message`; drive from the shared fixtures. `tests/legacy.test.ts` needs the coherent-policy edit |
| L2 | Tests (codec) | Missing-tuning-field defaults (one per group), `Auth_Mode` blank error, plain-text blank errors, datastream profiles as fixtures (`fixtures/datastream/<profile>/`) |
| L3 | Tests (capture) | Typed model, warnings, priority order; after 7-11 |
| L4 | CI | Validate `fixtures/messages/client_config/*.json` against the message schema; consider `additionalProperties: false` |
| L5 | Docs | Finalise README / REVIEW_NOTES / CONSUMERS / TRIGGER_CONFIG_DESIGN; reconcile DESIGN.md (see REVIEW_NOTES, Where this departs from DESIGN.md) |
| L6 | Open decision | Rename `SamplingInterval::ServerNegotiated` (`-1` means "inherit the subscription's publishing interval") |
