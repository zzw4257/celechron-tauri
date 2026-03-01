#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();

function readJson(p) {
  return JSON.parse(fs.readFileSync(p, 'utf8'));
}

function writeJson(p, data) {
  fs.writeFileSync(p, JSON.stringify(data, null, 2) + '\n');
}

function replaceInFile(filePath, replacer) {
  const abs = path.join(ROOT, filePath);
  const before = fs.readFileSync(abs, 'utf8');
  const after = replacer(before);
  if (after !== before) {
    fs.writeFileSync(abs, after);
    console.log(`updated ${filePath}`);
  }
}

const tauriConfPath = path.join(ROOT, 'src-tauri', 'tauri.conf.json');
const tauriConf = readJson(tauriConfPath);
const version = tauriConf.version;

if (!/^\d+\.\d+\.\d+/.test(version)) {
  throw new Error(`Unsupported version format: ${version}`);
}

const [major, minor, patch] = version.split('.').map((n) => Number.parseInt(n, 10));
const androidVersionCode = major * 1_000_000 + minor * 1_000 + patch;

const packageJsonPath = path.join(ROOT, 'package.json');
const packageJson = readJson(packageJsonPath);
if (packageJson.version !== version) {
  packageJson.version = version;
  writeJson(packageJsonPath, packageJson);
  console.log('updated package.json');
}

replaceInFile('src-tauri/Cargo.toml', (src) =>
  src.replace(/(?<=^version\s*=\s*")([^"]+)(?="\s*$)/m, version)
);

replaceInFile('src-tauri/gen/android/app/tauri.properties', (src) => {
  let out = src.replace(/tauri\.android\.versionName=.*/g, `tauri.android.versionName=${version}`);
  out = out.replace(/tauri\.android\.versionCode=.*/g, `tauri.android.versionCode=${androidVersionCode}`);
  return out;
});

replaceInFile('src-tauri/gen/apple/project.yml', (src) => {
  let out = src.replace(/CFBundleShortVersionString:\s*[^\n]+/g, `CFBundleShortVersionString: ${version}`);
  out = out.replace(/CFBundleVersion:\s*"?[^"]+"?/g, `CFBundleVersion: "${version}"`);
  return out;
});

console.log(`version sync completed -> ${version}`);
