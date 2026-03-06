#!/usr/bin/env node
import { spawnSync } from 'node:child_process';

const required = ['ZJU_USERNAME', 'ZJU_PASSWORD'];
const missing = required.filter((key) => !process.env[key]);
if (missing.length > 0) {
  console.error(`Missing env vars: ${missing.join(', ')}`);
  console.error('Usage: ZJU_USERNAME=... ZJU_PASSWORD=... npm run smoke:local');
  process.exit(1);
}

const result = spawnSync(
  'cargo',
  ['test', '--manifest-path', 'src-tauri/Cargo.toml', 'local_dev_smoke_report', '--', '--ignored', '--nocapture'],
  {
    stdio: 'inherit',
    env: process.env,
  },
);

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
