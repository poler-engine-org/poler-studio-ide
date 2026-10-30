#!/usr/bin/env node
/*---------------------------------------------------------------------------------------------
 *  POLER Studio build step: product.json checksum maintenance.
 *
 *  Stage D.1 of the distribution invariants: any modification under
 *  `out/vs/**` must be accompanied by a recomputation of the Base64
 *  SHA-256 checksums in product.json, otherwise the runtime
 *  IntegrityService flags the installation as modified (isPure: false)
 *  and the editor shows the "installation appears to be corrupt"
 *  notification.
 *
 *  The checksum list mirrors build/gulpfile.vscode.ts::packageTask —
 *  the ten entry-point files validated at startup:
 *
 *    vs/base/parts/sandbox/electron-browser/preload.js
 *    vs/workbench/workbench.desktop.main.js
 *    vs/workbench/workbench.desktop.main.css
 *    vs/workbench/api/node/extensionHostProcess.js
 *    vs/code/electron-browser/workbench/workbench.html
 *    vs/code/electron-browser/workbench/workbench.js
 *    vs/sessions/sessions.desktop.main.js
 *    vs/sessions/sessions.desktop.main.css
 *    vs/sessions/electron-browser/sessions.html
 *    vs/sessions/electron-browser/sessions.js
 *
 *  Usage (from the repository root, after packaging and any out/vs edits):
 *      node build/poler-studio/update-checksums.mjs <packaged-app-dir>            # rewrite
 *      node build/poler-studio/update-checksums.mjs <packaged-app-dir> --verify   # CI gate
 *
 *  `--verify` recomputes everything and exits non-zero on any mismatch,
 *  including when product.json carries no checksums at all.
 *--------------------------------------------------------------------------------------------*/

import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';

const CHECKSUMMED_FILES = [
	'vs/base/parts/sandbox/electron-browser/preload.js',
	'vs/workbench/workbench.desktop.main.js',
	'vs/workbench/workbench.desktop.main.css',
	'vs/workbench/api/node/extensionHostProcess.js',
	'vs/code/electron-browser/workbench/workbench.html',
	'vs/code/electron-browser/workbench/workbench.js',
	'vs/sessions/sessions.desktop.main.js',
	'vs/sessions/sessions.desktop.main.css',
	'vs/sessions/electron-browser/sessions.html',
	'vs/sessions/electron-browser/sessions.js'
];

/**
 * Base64 SHA-256 with the trailing `=` padding trimmed, matching
 * build/gulpfile.vscode.ts::computeChecksum exactly.
 */
function computeChecksum(filename) {
	const contents = fs.readFileSync(filename);
	return crypto
		.createHash('sha256')
		.update(contents)
		.digest('base64')
		.replace(/=+$/, '');
}

function computeChecksums(outDir) {
	const result = {};
	for (const filename of CHECKSUMMED_FILES) {
		const fullPath = path.join(outDir, filename);
		result[filename] = computeChecksum(fullPath);
	}
	return result;
}

function main() {
	const verify = process.argv.includes('--verify');
	const appDir = process.argv.find((arg, index) => index > 1 && !arg.startsWith('--'));
	if (!appDir) {
		console.error('usage: node build/poler-studio/update-checksums.mjs <packaged-app-dir> [--verify]');
		process.exit(2);
	}

	// The workflow packages the app as `<dir>/resources/app` (with `out`
	// and `product.json` inside); accept both the build dir and the app dir.
	const candidates = [appDir, path.join(appDir, 'resources', 'app')];
	const appRoot = candidates.find(candidate => fs.existsSync(path.join(candidate, 'out', 'vs')));
	if (!appRoot) {
		console.error(`update-checksums: no out/vs directory under ${appDir}`);
		process.exit(2);
	}

	const productJsonPath = path.join(appRoot, 'product.json');
	const product = JSON.parse(fs.readFileSync(productJsonPath, 'utf8'));
	const fresh = computeChecksums(path.join(appRoot, 'out'));
	const previous = product.checksums ?? {};

	const mismatches = [];
	for (const [filename, checksum] of Object.entries(fresh)) {
		if (previous[filename] !== checksum) {
			mismatches.push(`  ${filename}: ${previous[filename] ?? '<absent>'} -> ${checksum}`);
		}
	}
	const staleEntries = Object.keys(previous).filter(filename => !(filename in fresh));

	if (verify) {
		if (mismatches.length || staleEntries.length || !product.checksums) {
			console.error(`update-checksums: verification FAILED — ${mismatches.length} mismatched, ${staleEntries.length} stale entries`);
			for (const line of mismatches) {
				console.error(line);
			}
			for (const filename of staleEntries) {
				console.error(`  ${filename}: stale (no longer packaged)`);
			}
			process.exit(1);
		}
		console.log('update-checksums: verification passed — 10/10 entry-point checksums match (isPure invariant holds)');
		return;
	}

	product.checksums = fresh;
	// Preserve the two-space indentation used by product.json in-tree.
	fs.writeFileSync(productJsonPath, JSON.stringify(product, null, '  ') + '\n', 'utf8');
	console.log(`update-checksums: ${mismatches.length} checksum(s) updated in ${path.relative(process.cwd(), productJsonPath)}`);
	for (const line of mismatches) {
		console.log(line);
	}
}

main();
