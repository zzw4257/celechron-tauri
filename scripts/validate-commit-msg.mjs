#!/usr/bin/env node
import fs from 'node:fs';

const file = process.argv[2];
if (!file) {
  console.error('Usage: validate-commit-msg <commit-msg-file>');
  process.exit(1);
}

const message = fs.readFileSync(file, 'utf8').trim();
const firstLine = message.split(/\r?\n/)[0] || '';
const pattern = /^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9-]+\))?: .+$/;

if (!pattern.test(firstLine)) {
  console.error('Commit message does not match required format:');
  console.error('  <type>(<scope>): <subject>');
  console.error('Example: feat(gpa): 统一后端 GPA 计算口径');
  process.exit(1);
}

if (/^[^a-z]/.test(firstLine)) {
  // Basic guard for emoji prefix and similar non-type starts.
  console.error('Commit message cannot start with emoji or non-type prefix.');
  process.exit(1);
}
