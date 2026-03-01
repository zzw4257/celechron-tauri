#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const BASELINE_PATH = path.join(ROOT, 'scripts', 'color-baseline.json');
const UPDATE_BASELINE = process.argv.includes('--update-baseline');

const targetDirs = [
  path.join(ROOT, 'src', 'components'),
];

const colorRegex = /#[0-9a-fA-F]{3,8}\b|rgba?\([^\)]*\)|hsla?\([^\)]*\)|color-mix\([^\)]*\)/g;
const ignoreLineRegex = /var\(--|gradient\(|url\(|\/\/|\/\*/;

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.isFile() && full.endsWith('.vue')) out.push(full);
  }
  return out;
}

function countLiterals(content) {
  const lines = content.split(/\r?\n/);
  let count = 0;
  for (const line of lines) {
    if (ignoreLineRegex.test(line)) continue;
    const matches = line.match(colorRegex);
    if (matches) count += matches.length;
  }
  return count;
}

const files = targetDirs.flatMap((d) => walk(d));
const stats = {};
for (const file of files) {
  const rel = path.relative(ROOT, file).replaceAll('\\', '/');
  const content = fs.readFileSync(file, 'utf8');
  stats[rel] = countLiterals(content);
}

if (UPDATE_BASELINE || !fs.existsSync(BASELINE_PATH)) {
  fs.writeFileSync(BASELINE_PATH, JSON.stringify(stats, null, 2) + '\n');
  console.log(`color baseline updated: ${path.relative(ROOT, BASELINE_PATH)}`);
  process.exit(0);
}

const baseline = JSON.parse(fs.readFileSync(BASELINE_PATH, 'utf8'));
const violations = [];

for (const [file, count] of Object.entries(stats)) {
  const base = baseline[file];
  if (base === undefined && count > 0) {
    violations.push(`${file}: new file has ${count} color literals (baseline missing)`);
  } else if (base !== undefined && count > base) {
    violations.push(`${file}: color literals increased ${base} -> ${count}`);
  }
}

if (violations.length > 0) {
  console.error('Hardcoded color check failed:');
  for (const v of violations) console.error(`- ${v}`);
  console.error('If intentional, run: node scripts/check-colors.mjs --update-baseline');
  process.exit(1);
}

console.log('Hardcoded color check passed (no baseline increase).');
