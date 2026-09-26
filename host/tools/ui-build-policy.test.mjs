// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { resolveConfig } from 'vite';

const root = fileURLToPath(new URL('../', import.meta.url));
const configuration = (mode, build = {}) => resolveConfig({ configFile: resolve(root, 'vite.config.ts'), mode, build }, 'build');

test('product and synthetic QA outputs stay separate', async () => {
  const product = await configuration('production');
  const qa = await configuration('qa');
  assert.equal(resolve(product.root, product.build.outDir), resolve(root, 'ui/dist'));
  assert.equal(resolve(qa.root, qa.build.outDir), resolve(root, 'target/ui-qa'));
  assert.equal(product.build.sourcemap, false);
});

test('QA cannot override its output to a product or arbitrary directory', async () => {
  for (const outDir of ['dist', 'DIST', '../elsewhere', '../ui/dist']) {
    await assert.rejects(configuration('qa', { outDir }), /may only build into target\/ui-qa/u);
  }
});

test('product builds reject every qa-prefixed module, on either path separator', async () => {
  const product = await configuration('production');
  const guard = product.plugins.find((plugin) => plugin.name === 'isolate-gallery-from-product');
  assert.equal(typeof guard?.generateBundle, 'function');
  const bundle = (id) => ({ 'entry.js': { type: 'chunk', modules: { [id]: {} } } });
  for (const id of ['/repo/ui/src/qa-main.tsx', 'C:\\repo\\ui\\src\\qa-fixtures.ts', '/repo/ui/src/qa-future.js?something', '/repo/ui/src/fixtures.ts']) {
    assert.throws(() => guard.generateBundle({}, bundle(id)), /forbidden in a product build/u);
  }
  assert.doesNotThrow(() => guard.generateBundle({}, bundle('/repo/ui/src/main.tsx')));
});
