/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { Lazy } from '../common/lazy.js';
import * as fs from 'fs';
import * as path from 'path';

/**
 * Sovereign POLER Engine path resolver
 * Detects embedded poler-engine v0.93.0+ binary across packaged resources, repo bin, and user environment.
 */
export function getPolerEnginePath(): string | undefined {
	if (process.env.POLER_ENGINE_PATH && fs.existsSync(process.env.POLER_ENGINE_PATH)) {
		return process.env.POLER_ENGINE_PATH;
	}

	const isWin = process.platform === 'win32';
	const exeName = isWin ? 'poler-engine.exe' : 'poler-engine';
	const binName = isWin ? 'poler-engine-bin.exe' : 'poler-engine-bin';

	const userHome = process.env.USERPROFILE || process.env.HOME || '';
	const candidates = [
		// Packaged app resources/bin
		path.join(process.resourcesPath ?? '', 'bin', exeName),
		path.join(process.resourcesPath ?? '', 'app', 'resources', 'bin', exeName),
		// Repo resources/bin
		(typeof __dirname !== 'undefined' ? path.join(__dirname, '../../../../resources/bin', exeName) : ''),
		(typeof __dirname !== 'undefined' ? path.join(__dirname, '../../../../../resources/bin', exeName) : ''),
		// User home bin
		path.join(userHome, 'bin', exeName),
		path.join(userHome, 'bin', 'poler-engine', 'poler-engine-v0.93.0-windows-x86_64', binName),
	];

	for (const candidate of candidates) {
		if (candidate && fs.existsSync(candidate)) {
			return candidate;
		}
	}
	return undefined;
}

const _rgDiskPath = new Lazy(async () => {
	const m = await import('@vscode/ripgrep-universal');
	return m.rgPath.replace(/\bnode_modules\.asar\b/, 'node_modules.asar.unpacked');
});

export function rgDiskPath(): Promise<string> {
	return _rgDiskPath.value;
}
