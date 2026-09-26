// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { spawnSync } from 'node:child_process';
import { statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
if (process.argv.length !== 2) throw new Error('Usage: node tools/ui-quality.mjs');
const steps = [
  ['node_modules/typescript/bin/tsc', '-b', '--pretty', 'false'],
  ['node_modules/eslint/bin/eslint.js', 'ui', '--max-warnings', '0'],
  ['node_modules/vitest/vitest.mjs', 'run'],
  ['node_modules/vite/bin/vite.js', 'build', '--mode', 'production'],
];
for (const args of steps) {
  console.log(`\n> node ${args.join(' ')}`);
  const result = spawnSync(process.execPath, args, { cwd: root, stdio: 'inherit', timeout: 300_000 });
  if (result.error || result.signal || result.status !== 0) {
    console.error(`UI check failed: ${result.error?.message ?? result.signal ?? result.status}`);
    process.exit(result.status && result.status > 0 ? result.status : 1);
  }
}
const entry = statSync(new URL('../ui/dist/index.html', import.meta.url));
if (!entry.isFile() || entry.size === 0) throw new Error('Product build must produce a non-empty ui/dist/index.html.');
console.log('UI checks passed. No browser or native application was launched.');
