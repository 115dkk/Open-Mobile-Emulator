// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
if (process.argv.length !== 2) throw new Error('Usage: node tools/quality.mjs');
const steps = [
  [process.execPath, ['tools/ui-quality.mjs']],
  [process.execPath, ['--test', 'tools/tauri-capability.test.mjs', 'tools/ui-build-policy.test.mjs']],
  ['cargo', ['fmt', '--all', '--', '--check']],
  ['cargo', ['clippy', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings']],
  ['cargo', ['test', '--workspace']],
];
for (const [command, args] of steps) {
  console.log(`\n> ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', timeout: 900_000 });
  if (result.error || result.signal || result.status !== 0) {
    console.error(`Quality gate failed: ${result.error?.message ?? result.signal ?? result.status}`);
    process.exit(result.status && result.status > 0 ? result.status : 1);
  }
}
console.log('Source and build gates passed. Guest operation requires separate evidence.');
