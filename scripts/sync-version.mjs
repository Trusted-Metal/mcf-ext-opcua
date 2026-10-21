#!/usr/bin/env node
// Copies the version from root package.json (already bumped by @semantic-release/npm)
// to the language manifests. Run by semantic-release as prepareCmd.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const { version } = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
if (!version) {
  console.error('No version found in package.json');
  process.exit(1);
}

function updateJson(rel, mutate) {
  const path = join(root, rel);
  if (!existsSync(path)) return;
  const json = JSON.parse(readFileSync(path, 'utf8'));
  mutate(json);
  writeFileSync(path, JSON.stringify(json, null, 2) + '\n');
  console.log(`  ${rel} -> ${version}`);
}

console.log(`Syncing version ${version}`);

updateJson('nodejs/package.json', (j) => {
  j.version = version;
});
updateJson('nodejs/package-lock.json', (j) => {
  j.version = version;
  if (j.packages?.['']) j.packages[''].version = version;
});

// Rust crate lives in rust/ once it exists; the first `version = ` line is the [package] one.
const cargoPath = join(root, 'rust', 'Cargo.toml');
if (existsSync(cargoPath)) {
  const cargo = readFileSync(cargoPath, 'utf8');
  writeFileSync(cargoPath, cargo.replace(/^(version\s*=\s*)"[^"]+"/m, `$1"${version}"`));
  console.log(`  rust/Cargo.toml -> ${version}`);
}
