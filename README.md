# mcf-ext-opcua

Pack/unpack codec for the OPC UA extension (`TM_OPCUA`) of a machine configuration file (`.h5`).

This repo is an **extension** of the [Machine Config Library](https://github.com/Trusted-Metal/Machine_Config_Library) ("MCF core"). Core reads and writes the HDF5 file; this library only turns the OPC UA part into a typed model and back. Pure functions, no I/O, no HDF5 dependency, no runtime dependencies.

## Contents

- [How it relates to MCF core](#how-it-relates-to-mcf-core)
- [Usage](#usage)
- [Repository layout](#repository-layout)
- [Development](#development)
- [CI and releases](#ci-and-releases)
- [Commit convention](#commit-convention)
- [Documentation](#documentation)

## How it relates to MCF core

```text
 .h5 file  <-- read / write -->  MCF core  --  MachineConfig.extensions["TM_OPCUA"]  -->  mcf-ext-opcua
                                 (HDF5)         (opaque ExtensionTree)                     unpack / pack
                                                                                           OpcuaConfig
```

| | MCF core | mcf-ext-opcua |
|---|---|---|
| Owns | HDF5 reads and writes, `File_Version`, the extension contract | The OPC UA schema, domain model, validation, legacy conversion |
| Knows about OPC UA | No (deprecated typed `opcua` model being removed) | Yes |
| Touches the file | Yes | Never |
| Version axis | `File_Version` and `EXTENSION_CONTRACT_VERSION` | Schema version (`1.2`) read from the extension's root group; `0.x` package version |

- Core carries every group under `Extensions/` as an opaque `ExtensionTree` (`attrs` / `children` / `datasets`) and round-trips it unchanged, including extensions it doesn't understand. The tree shape and its canonical digest are defined by the contract in core's `extension-contract/` (Rust crate `mcf-extension-contract`, Node types-only subpath).
- This library declares the tree types structurally, so it has no dependency on core, in Node or Rust. It states the contract range it needs as `REQUIRES_CONTRACT`; the host checks compatibility at runtime. Core and extensions release independently: nothing is bumped in lockstep, and anyone can publish another extension against the same contract without a core release.
- Trees written by core's old typed OPC UA support have no schema version attribute. They are read through a dedicated legacy adapter and come out as the same model (flat triggers become start/stop rules).
- The extension contract is under development in core (branch `feat/extension-contract`). Until it ships, [DESIGN.md](DESIGN.md) sections 1-3 are the working definition.

## Usage

```ts
import { unpack, pack, EXTENSION_NAME } from 'mcf-ext-opcua';

const tree = config.extensions[EXTENSION_NAME];          // config: MachineConfig from core
const r = unpack(tree);                                  // Result<OpcuaConfig, ExtensionError>
if (!r.ok) throw new Error(`${r.error.code} at ${r.error.path}: ${r.error.message}`);

r.value.triggers.rules[0].enabled = false;

const out = pack(r.value);                               // always writes the current schema (1.2)
if (out.ok) config.extensions[EXTENSION_NAME] = out.value;
```

See [nodejs/README.md](nodejs/README.md) for the full API. A Rust crate with the same interface is planned (`rust/`). It will define its own tree types rather than depend on core's contract crate.

## Repository layout

```text
fixtures/    shared test fixtures (tree.json + model.json), read by every language's tests
nodejs/      npm package `mcf-ext-opcua` (TypeScript, vitest)
rust/        crate `mcf-ext-opcua` (planned)
tools/       extract-legacy-fixture.mjs: dumps a legacy tree from core's reference .h5
scripts/     sync-version.mjs: release tooling
DESIGN.md    design and decisions
```

Fixtures are the cross-language contract: each implementation must `unpack(tree) == model` and `pack(model) == tree` on the same files, so Node and Rust cannot drift.

## Development

```bash
cd nodejs
npm ci
npm run typecheck
npm test
npm run build
```

## CI and releases

| Workflow | File | Trigger | What it does |
|---|---|---|---|
| **Node.js** | `nodejs.yml` | Every push to any branch | Typecheck, build and vitest on Ubuntu + Windows. Then packs the tarball, installs it into a clean project to check it imports, and uploads it as a 14-day artifact (`mcf-ext-opcua-nodejs-tgz`) |
| **Rust** | `rust.yml` | Push touching `rust/`, `fixtures/`, `schema/` | `cargo fmt`, `clippy`, `cargo test` on Ubuntu + Windows. Does nothing until `rust/` exists |
| **Release** | `release.yml` | Push to `main` or `release` | Typecheck + tests, then `semantic-release`: bumps the version, updates `CHANGELOG.md`, tags `vX.Y.Z`, and creates a GitHub release with the Node tarball attached |

Versioning is lockstep across languages and follows the commit history:

- `main` publishes `rc` pre-releases (`0.1.0-rc.N`); `release` publishes stable versions.
- `scripts/sync-version.mjs` copies the version to `nodejs/package.json` (and the lockfile), and to `rust/Cargo.toml` when it exists.
- The release commit is pushed with a deploy key (`RELEASE_DEPLOY_KEY` repository secret) so it can bypass branch protection on `main` and `release`.

### Consuming a release

Node.js: point `package.json` at the release asset, so a plain `npm install` or `pnpm install` works with no registry:

```json
"mcf-ext-opcua": "https://github.com/Trusted-Metal/mcf-ext-opcua/releases/download/v0.1.0-rc.1/mcf-ext-opcua-0.1.0-rc.1.tgz"
```

Rust: depend on the git tag (the crate has no registry release):

```toml
mcf-ext-opcua = { git = "https://github.com/Trusted-Metal/mcf-ext-opcua", tag = "v0.1.0-rc.1" }
```

To try a branch before release, download the `mcf-ext-opcua-nodejs-tgz` artifact from its Node.js workflow run.

## Commit convention

[Conventional Commits](https://www.conventionalcommits.org/), enforced locally by commitlint (husky `commit-msg` hook). **Scopes** (warning only): `nodejs`, `rust`, `schema`, `fixtures`, `ci`, `docs`.

| Type | Version bump |
|---|---|
| `feat` | minor |
| `fix`, `perf`, `refactor`, `revert` | patch |
| `docs`, `style`, `test`, `build`, `ci`, `chore` | none |
| `BREAKING CHANGE:` footer | major |

A commit that only changes CI or packaging does not release. Use `fix` or `feat` if you need a version bump to test the release pipeline.

## Documentation

| Document | Contents |
|---|---|
| [DESIGN.md](DESIGN.md) | Extension contract, domain model, adapters, schema, testing, release |
| [nodejs/README.md](nodejs/README.md) | Node.js API |
