#!/usr/bin/env node
/*---------------------------------------------------------------------------------------------
 *  POLER Studio build step: provision sovereign extension stubs.
 *
 *  Stage D.2 of the distribution invariants: every extension stub required
 *  by the bundled Copilot extension (@github/copilot/sdk, dotenv) is
 *  generated at build time instead of being committed to the repository.
 *  The generated files land in the packaged application tree, so `out/vs`
 *  and the committed sources both stay free of vendored placeholders.
 *
 *  Usage (from the repository root, after `gulp vscode-win32-x64`):
 *      node build/poler-studio/embed-stubs.mjs <packaged-app-dir>
 *      node build/poler-studio/embed-stubs.mjs <packaged-app-dir> --check
 *
 *  `--check` is a dry run that exits non-zero when any stub is missing or
 *  out of date, so CI can gate on it.
 *--------------------------------------------------------------------------------------------*/

import * as fs from 'fs';
import * as path from 'path';

/**
 * The @github/copilot/sdk compatibility stub: only the surface actually
 * imported by the bundled Copilot extension at runtime. Kept in sync with
 * the imports audit in .github/workflows/build-poler-studio.yml.
 */
const COPILOT_SDK_INDEX_JS = `export class AutoModeSessionManager { constructor(opts) {} }
export class LocalSessionManager {
  constructor(opts) {}
  async listSessions() { return []; }
  async getSessions() { return []; }
  async getSession() { return null; }
  async deleteSession() {}
}
export class NoopTelemetryService {}
export function createLocalFeatureFlagServiceCreator() { return () => ({}); }
export async function getAvailableModels(authInfo) { return []; }
export const logger = { setLogWriter: () => {} };
export const internal = { LocalSessionManager, NoopTelemetryService };
`;

const COPILOT_SDK_PACKAGE_JSON = `{
  "name": "@github/copilot",
  "version": "1.0.73",
  "type": "module",
  "exports": {
    "./sdk": "./sdk/index.js"
  }
}
`;

const DOTENV_INDEX_JS = `module.exports = { config: () => ({ parsed: {} }) };
`;

const DOTENV_PACKAGE_JSON = `{
  "name": "dotenv",
  "version": "16.4.5",
  "main": "index.js"
}
`;

/**
 * Every stub: [relative target path, generated content, description].
 */
const STUBS = [
	['extensions/copilot/node_modules/@github/copilot/sdk/index.js', COPILOT_SDK_INDEX_JS, '@github/copilot/sdk entry point'],
	['extensions/copilot/node_modules/@github/copilot/package.json', COPILOT_SDK_PACKAGE_JSON, '@github/copilot package manifest'],
	['extensions/copilot/node_modules/dotenv/index.js', DOTENV_INDEX_JS, 'dotenv entry point'],
	['extensions/copilot/node_modules/dotenv/package.json', DOTENV_PACKAGE_JSON, 'dotenv package manifest']
];

function main() {
	const checkOnly = process.argv.includes('--check');
	const appDir = process.argv.find((arg, index) => index > 1 && !arg.startsWith('--'));
	if (!appDir) {
		console.error('usage: node build/poler-studio/embed-stubs.mjs <packaged-app-dir> [--check]');
		process.exit(2);
	}

	// The workflow packages the app with `resources/app` inside the build
	// directory; accept both the build dir and the app dir itself.
	const candidates = [appDir, path.join(appDir, 'resources', 'app')];
	const appRoot = candidates.find(candidate => fs.existsSync(path.join(candidate, 'extensions')));
	if (!appRoot) {
		console.error(`embed-stubs: no extensions/ directory under ${appDir}`);
		process.exit(2);
	}

	const changed = [];
	for (const [relativePath, content, description] of STUBS) {
		const target = path.join(appRoot, relativePath);
		const existing = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : undefined;
		if (existing === content) {
			console.log(`embed-stubs: ${description} already up to date (${relativePath})`);
			continue;
		}
		changed.push(relativePath);
		console.log(`embed-stubs: ${checkOnly ? '[dry-run] ' : ''}writing ${description} -> ${relativePath}`);
		if (!checkOnly) {
			fs.mkdirSync(path.dirname(target), { recursive: true });
			fs.writeFileSync(target, content, 'utf8');
		}
	}

	if (checkOnly && changed.length) {
		console.error(`embed-stubs: ${changed.length} stub file(s) out of date; run without --check to regenerate`);
		process.exit(1);
	}
	console.log(`embed-stubs: ${changed.length} file(s) ${checkOnly ? 'would be ' : ''}generated`);
}

main();
