/*---------------------------------------------------------------------------------------------
 *  POLER Studio IDE - unit tests for the sovereign search gateway helpers
 *--------------------------------------------------------------------------------------------*/

import assert from 'assert';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { ensureNoDisposablesAreLeakedInTestSuite } from '../../../../../base/test/common/utils.js';
import { expandRootExcludingPruned, literalPrunedDirName, resolveMultiTierScanFolder } from '../../node/polerTextSearchEngine.js';

suite('PolerTextSearchEngine helpers', () => {
        ensureNoDisposablesAreLeakedInTestSuite();

        suite('literalPrunedDirName', () => {
                test('whole-tree globs reduce to the bare directory name', () => {
                        assert.strictEqual(literalPrunedDirName('**/node_modules/**'), 'node_modules');
                        assert.strictEqual(literalPrunedDirName('**/node_modules'), 'node_modules');
                        assert.strictEqual(literalPrunedDirName('node_modules'), 'node_modules');
                        assert.strictEqual(literalPrunedDirName('node_modules/**'), 'node_modules');
                        assert.strictEqual(literalPrunedDirName('**/bower_components'), 'bower_components');
                });

                test('multi-segment or wildcard globs prune nothing', () => {
                        assert.strictEqual(literalPrunedDirName('Документы/Cassiopeia/**'), undefined);
                        assert.strictEqual(literalPrunedDirName('**/*.code-search'), undefined);
                        assert.strictEqual(literalPrunedDirName('**/build/*.tmp'), undefined);
                        assert.strictEqual(literalPrunedDirName('**'), undefined);
                        assert.strictEqual(literalPrunedDirName(''), undefined);
                });

                test('RelativePattern-like objects are handled by the caller, not here', () => {
                        // plain strings only; the collector unwraps {baseUri,pattern}
                        assert.strictEqual(literalPrunedDirName('dist'), 'dist');
                });
        });

        suite('expandRootExcludingPruned', () => {
                const root = fs.mkdtempSync(path.join(os.tmpdir(), 'poler-expand-'));

                test('no pruning set returns the bare root', () => {
                        assert.deepStrictEqual(expandRootExcludingPruned(root, new Set()), [root]);
                });

                test('pruned top-level directories are dropped, the rest becomes scan units', () => {
                        fs.mkdirSync(path.join(root, 'node_modules'));
                        fs.mkdirSync(path.join(root, 'src'));
                        fs.writeFileSync(path.join(root, 'README.md'), 'x');

                        const units = expandRootExcludingPruned(root, new Set(['node_modules']));
                        assert.deepStrictEqual(units.sort(), [
                                path.join(root, 'README.md'),
                                path.join(root, 'src'),
                        ]);
                });

                test('hidden entries are skipped like the engine walk skips them', () => {
                        fs.mkdirSync(path.join(root, '.git'));
                        const units = expandRootExcludingPruned(root, new Set());
                        assert.ok(!units.some(u => u.endsWith('.git')));
                });

                test('a fully pruned root falls back to the bare root', () => {
                        const onlyJunk = fs.mkdtempSync(path.join(os.tmpdir(), 'poler-junk-'));
                        fs.mkdirSync(path.join(onlyJunk, 'node_modules'));
                        assert.deepStrictEqual(expandRootExcludingPruned(onlyJunk, new Set(['node_modules'])), [onlyJunk]);
                });
        });

        suite('resolveMultiTierScanFolder', () => {
                const root = fs.mkdtempSync(path.join(os.tmpdir(), 'poler-tier-'));

                test('tier 1 resolves a literal include subpath', () => {
                        fs.mkdirSync(path.join(root, 'src', 'vs', 'base'), { recursive: true });
                        assert.strictEqual(resolveMultiTierScanFolder(root, ['src/vs/base/**/*.ts']), path.join(root, 'src', 'vs', 'base'));
                });

                test('tier 2/3 resolves the deepest matching include segment', () => {
                        fs.mkdirSync(path.join(root, 'Cassiopeia'));
                        assert.strictEqual(resolveMultiTierScanFolder(root, ['some/other/Cassiopeia/**/*.py']), path.join(root, 'Cassiopeia'));
                });

                test('no includes or unresolvable includes keep the root', () => {
                        assert.strictEqual(resolveMultiTierScanFolder(root, undefined), root);
                        assert.strictEqual(resolveMultiTierScanFolder(root, ['**/*.py']), root);
                });
        });
});
