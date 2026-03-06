#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const ROOT = process.cwd();
const tauriConf = JSON.parse(fs.readFileSync(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const version = tauriConf.version;
const explicitTag = process.env.CELECHRON_RELEASE_TAG?.trim() || '';

function safeGit(args) {
  try {
    return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8' }).trim();
  } catch {
    return '';
  }
}

const commit = safeGit(['rev-parse', '--short', 'HEAD']);
const headTag = explicitTag || safeGit(['tag', '--points-at', 'HEAD']).split('\n').filter(Boolean)[0] || '';
const tag = headTag || null;
const channel = tag
  ? (/^v\d+\.\d+\.\d+$/.test(tag) ? 'stable' : 'dev')
  : 'local';
const title = tag || version;
const artifactPrefix = `Celechron-${tag || `v${version}-local`}`;

const manifest = {
  productName: tauriConf.productName,
  version,
  tag,
  title,
  channel,
  commit,
  builtAt: new Date().toISOString(),
  targets: ['macos', 'android'],
  artifactPrefix,
};

const distDir = path.join(ROOT, 'dist');
fs.mkdirSync(distDir, { recursive: true });
const outPath = path.join(distDir, 'release-manifest.json');
fs.writeFileSync(outPath, JSON.stringify(manifest, null, 2) + '\n');
console.log(`release manifest written -> ${path.relative(ROOT, outPath)}`);
