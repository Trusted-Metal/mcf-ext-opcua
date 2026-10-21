// One-off dev script: dumps extensions.TM_OPCUA of core's reference_config_opcua.h5 as tree.json.
// Core is used here only to read the file; it is not a dependency of the package.
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const CORE_DIST = 'C:/repo/Machine_Config_Library/nodejs/dist/index.js';
const H5 = 'C:/repo/Machine_Config_Library/fixtures/reference_config_opcua.h5';
const OUT = resolve(import.meta.dirname, '../fixtures/legacy/reference_config_opcua/tree.json');

const { MachineConfigReader, extensionTreeToJson } = await import(pathToFileURL(CORE_DIST).href);
const config = await new MachineConfigReader(H5).parse();
const tree = config.extensions?.TM_OPCUA;
if (!tree) throw new Error('extensions.TM_OPCUA not found in ' + H5);

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, JSON.stringify(extensionTreeToJson(tree), null, 2) + '\n');
console.log('wrote', OUT);
