# mcf-ext-opcua

Pack/unpack codec for the OPC UA extension (`TM_OPCUA`) of a machine configuration file. Pure functions, no I/O, no runtime dependencies. It works on the extension tree that Machine Config Library core carries opaquely (types are declared structurally; core is not imported).

```ts
import { unpack, pack, peekSchemaVersion, EXTENSION_NAME } from 'mcf-ext-opcua';

const tree = config.extensions[EXTENSION_NAME];
peekSchemaVersion(tree); // '1.2' | 'legacy' | ...

const r = unpack(tree); // Result<OpcuaConfig, ExtensionError>
if (!r.ok) throw new Error(`${r.error.code} at ${r.error.path}: ${r.error.message}`);

r.value.triggers.rules[0].enabled = false;

const out = pack(r.value); // Result<ExtensionTree, ExtensionError>
if (out.ok) config.extensions[EXTENSION_NAME] = out.value;
```

- Current schema is `1.2`; `pack` always writes it. Any `1.x` is read, unknown attributes are kept in `extra`.
- A tree without `Opcua_Schema_Version` is the legacy layout written by core's old typed OPC UA support; it is converted to the same model (flat triggers become Start/Stop rules, skipped sides are listed in `warnings`).
- Packing to the legacy layout is not implemented (`pack(config, { targetSchema: 'legacy' })` returns `LossyDowngrade`).
- Errors carry `code` (`Validation`, `UnsupportedSchemaVersion`, `LossyDowngrade`) and a slash-joined `path`, e.g. `Trigger/Job Start/Condition 1/@Value`.
- `suggestedDefaults(profile?)` returns documented defaults for building a new config; `pack` never applies them.
