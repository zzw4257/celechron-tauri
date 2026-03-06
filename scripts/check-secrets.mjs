#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const ROOT = process.cwd();
const SKIP_FILES = new Set([
  'scripts/check-secrets.mjs',
]);
const BLOCKED_EXTENSIONS = new Set(['.jks', '.keystore', '.p12', '.mobileprovision']);
const TEXT_SIZE_LIMIT = 1024 * 1024;

const PATTERNS = [
  {
    name: 'cookie',
    regex: /\biPlanetDirectoryPro=[^;\s'"`]{8,}|\bJSESSIONID=[^;\s'"`]{8,}|\bsession=[A-Za-z0-9%+/_\-.=]{16,}/,
  },
  {
    name: 'private-key',
    regex: /-----BEGIN (?:RSA |EC |OPENSSH |)PRIVATE KEY-----|-----BEGIN CERTIFICATE-----/,
  },
  {
    name: 'literal-secret-env',
    regex: /\b(?:ZJU_USERNAME|ZJU_PASSWORD|ANDROID_KEYSTORE_PATH|ANDROID_KEYSTORE_PASSWORD|ANDROID_KEY_ALIAS|ANDROID_KEY_PASSWORD|APPLE_CERTIFICATE_PASSWORD|APPLE_TEAM_ID|APPLE_SIGNING_IDENTITY|DINGTALK_WEBHOOK|ZEROCLAW_API_KEY)\b\s*[:=]\s*(?:['"](?!\s*(?:你的|your|example|<|\$\{|process\.env|std::env::var|System\.getenv|env\.))[^"]+['"]|(?!\$\{|process\.env|std::env::var|System\.getenv|env\.)[^\s#]{6,})/,
  },
];

function getTrackedFiles() {
  const raw = execFileSync('git', ['ls-files', '-z'], { cwd: ROOT, encoding: 'buffer' });
  return raw
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .filter((file) => !SKIP_FILES.has(file));
}

function isBinary(buffer) {
  return buffer.includes(0);
}

function findLineNumber(content, index) {
  return content.slice(0, index).split('\n').length;
}

const findings = [];

for (const relativePath of getTrackedFiles()) {
  const ext = path.extname(relativePath).toLowerCase();
  if (BLOCKED_EXTENSIONS.has(ext)) {
    findings.push({
      file: relativePath,
      line: 1,
      reason: `blocked file extension ${ext}`,
    });
    continue;
  }

  const absPath = path.join(ROOT, relativePath);
  if (!fs.existsSync(absPath)) {
    continue;
  }

  const stat = fs.statSync(absPath);
  if (!stat.isFile() || stat.size > TEXT_SIZE_LIMIT) {
    continue;
  }

  const buffer = fs.readFileSync(absPath);
  if (isBinary(buffer)) {
    continue;
  }

  const content = buffer.toString('utf8');
  for (const pattern of PATTERNS) {
    const match = pattern.regex.exec(content);
    if (!match) {
      continue;
    }
    findings.push({
      file: relativePath,
      line: findLineNumber(content, match.index),
      reason: pattern.name,
      sample: match[0].slice(0, 120),
    });
    break;
  }
}

if (findings.length > 0) {
  console.error('Potential secret leakage detected:');
  for (const finding of findings) {
    const sample = finding.sample ? ` -> ${finding.sample}` : '';
    console.error(`- ${finding.file}:${finding.line} [${finding.reason}]${sample}`);
  }
  process.exit(1);
}

console.log('Secret leakage check passed.');
