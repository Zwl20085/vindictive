#!/usr/bin/env node
// Keep the app version in sync across package.json, src-tauri/Cargo.toml and
// src-tauri/tauri.conf.json (plus both lockfiles).
//
//   npm run bump -- 0.4.1                     set every version to 0.4.1
//   node scripts/bump-version.mjs --check 0.4.1   exit 1 unless all equal 0.4.1
//
// Run from the repository root (npm run does that for you).

import { execSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const PACKAGE_JSON = join(ROOT, 'package.json');
const CARGO_TOML = join(ROOT, 'src-tauri', 'Cargo.toml');
const TAURI_CONF = join(ROOT, 'src-tauri', 'tauri.conf.json');
const CRATE_NAME = 'vindictive';

// Semver 2.0.0, without build metadata (Windows installers reject it).
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z.-]+)?$/;
// The first `version = "..."` inside the [package] table.
const CARGO_VERSION = /^(\[package\][^[]*?^version\s*=\s*")([^"]+)(")/m;
// The top-level "version" key of tauri.conf.json (two-space indented).
const TAURI_VERSION = /^(  "version"\s*:\s*")([^"]+)(")/m;

function fail(message) {
  console.error(`bump-version: ${message}`);
  process.exit(1);
}

function read(path) {
  return readFileSync(path, 'utf8');
}

function cargoVersion() {
  const m = read(CARGO_TOML).match(CARGO_VERSION);
  if (!m) fail(`no [package] version in ${CARGO_TOML}`);
  return m[2];
}

function tauriVersion() {
  return JSON.parse(read(TAURI_CONF)).version;
}

function currentVersions() {
  return {
    'package.json': JSON.parse(read(PACKAGE_JSON)).version,
    'src-tauri/Cargo.toml': cargoVersion(),
    'src-tauri/tauri.conf.json': tauriVersion(),
  };
}

function check(expected) {
  const versions = currentVersions();
  const wrong = Object.entries(versions).filter(([, v]) => v !== expected);
  for (const [file, v] of Object.entries(versions)) {
    console.log(`${v === expected ? 'ok  ' : 'FAIL'} ${file}: ${v}`);
  }
  if (wrong.length > 0) fail(`expected every version to be ${expected}`);
}

function replaceIn(path, pattern, version) {
  const text = read(path);
  if (!pattern.test(text)) fail(`no version field found in ${path}`);
  writeFileSync(path, text.replace(pattern, `$1${version}$3`));
}

// Runs through the shell because npm is a .cmd shim on Windows. Every
// argument is a constant or a version already checked against SEMVER, so
// nothing here needs quoting.
function run(command) {
  console.log(`> ${command}`);
  execSync(command, { cwd: ROOT, stdio: 'inherit' });
}

function bump(version) {
  run(`npm version ${version} --no-git-tag-version --allow-same-version`);
  replaceIn(CARGO_TOML, CARGO_VERSION, version);
  replaceIn(TAURI_CONF, TAURI_VERSION, version);
  if (tauriVersion() !== version) fail(`could not set the version in ${TAURI_CONF}`);
  // Rewrites the crate's own entry in Cargo.lock; touches nothing else.
  run(`cargo update -p ${CRATE_NAME} --manifest-path src-tauri/Cargo.toml`);
  check(version);
  console.log(`\nVersion is now ${version}. Next: update CHANGELOG.md, commit, tag v${version}.`);
}

const args = process.argv.slice(2);
const checking = args[0] === '--check';
const version = (checking ? args[1] : args[0])?.replace(/^v/, '');
if (!version || args.length > (checking ? 2 : 1)) {
  fail('usage: bump-version.mjs [--check] <version>, e.g. 0.4.1');
}
if (!SEMVER.test(version)) fail(`"${version}" is not a semver version like 0.4.1`);

if (checking) check(version);
else bump(version);
