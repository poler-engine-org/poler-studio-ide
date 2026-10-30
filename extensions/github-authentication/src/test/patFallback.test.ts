/*---------------------------------------------------------------------------------------------
 *  POLER Studio - Service PAT fallback tests (Stage C.1)
 *
 *  When the stored OAuth session is rejected with 401/403, readSessions
 *  retries with the service Personal Access Token (GITHUB_TOKEN env) and
 *  keeps the session alive with the PAT as its access token.
 *--------------------------------------------------------------------------------------------*/

import * as assert from 'assert';
import * as vscode from 'vscode';
import { GitHubSessionEngine } from '../github';
import { isUnauthorizedError } from '../common/patFallback';

interface TestGitHubAuthenticationProvider {
	readonly _keychain: {
		getToken(): Promise<string>;
		deleteToken(): Promise<void>;
	};
	readonly _githubServer: {
		getFallbackBaseUri(): vscode.Uri;
		getUserInfo(token: string): Promise<{ id: string; accountName: string; avatarUrl: string | undefined }>;
	};
	readonly _logger: {
		error(message: string): void;
		info(message: string): void;
		trace(message: string): void;
	};
	readSessions(): Promise<vscode.AuthenticationSession[]>;
}

suite('Service PAT fallback', () => {

	const storedSessions = JSON.stringify([{
		id: 'session-id',
		scopes: ['repo'],
		accessToken: 'expired-oauth-token'
	}]);

	setup(() => {
		process.env.GITHUB_TOKEN = 'service-pat';
	});

	teardown(() => {
		delete process.env.GITHUB_TOKEN;
		delete process.env.GH_TOKEN;
		delete process.env.GITHUB_PAT;
	});

	test('revoked OAuth token is replaced by the service PAT', async () => {
		const seenTokens: string[] = [];
		const provider: TestGitHubAuthenticationProvider = Object.assign(Object.create(GitHubSessionEngine.prototype), {
			_keychain: {
				getToken: async () => storedSessions,
				deleteToken: async () => { }
			},
			_githubServer: {
				getFallbackBaseUri: () => vscode.Uri.parse('https://github.com'),
				getUserInfo: async token => {
					seenTokens.push(token);
					if (token === 'expired-oauth-token') {
						throw new Error('Bad credentials');
					}
					return {
						id: 'account-id',
						accountName: 'octocat',
						avatarUrl: 'https://avatars.githubusercontent.com/u/1'
					};
				}
			},
			_logger: {
				error: _message => { },
				info: _message => { },
				trace: _message => { }
			},
			readSessions: GitHubSessionEngine.prototype['readSessions']
		} satisfies TestGitHubAuthenticationProvider);

		const sessions = await provider.readSessions();

		assert.strictEqual(sessions.length, 1, 'session must survive through the PAT fallback');
		assert.strictEqual(sessions[0].accessToken, 'service-pat', 'session must carry the service PAT');
		assert.deepStrictEqual(seenTokens, ['expired-oauth-token', 'service-pat'], 'OAuth token tried first, PAT second');
		assert.strictEqual(sessions[0].account.label, 'octocat');
	});

	test('session is dropped when no service PAT is configured', async () => {
		delete process.env.GITHUB_TOKEN;
		const provider: TestGitHubAuthenticationProvider = Object.assign(Object.create(GitHubSessionEngine.prototype), {
			_keychain: {
				getToken: async () => storedSessions,
				deleteToken: async () => { }
			},
			_githubServer: {
				getFallbackBaseUri: () => vscode.Uri.parse('https://github.com'),
				getUserInfo: async _token => { throw new Error('Unauthorized'); }
			},
			_logger: {
				error: _message => { },
				info: _message => { },
				trace: _message => { }
			},
			readSessions: GitHubSessionEngine.prototype['readSessions']
		} satisfies TestGitHubAuthenticationProvider);

		const sessions = await provider.readSessions();
		assert.strictEqual(sessions.length, 0, 'no fallback token -> session dropped (original behavior)');
	});

	test('session is dropped when the service PAT is also rejected', async () => {
		const provider: TestGitHubAuthenticationProvider = Object.assign(Object.create(GitHubSessionEngine.prototype), {
			_keychain: {
				getToken: async () => storedSessions,
				deleteToken: async () => { }
			},
			_githubServer: {
				getFallbackBaseUri: () => vscode.Uri.parse('https://github.com'),
				getUserInfo: async _token => { throw new Error('Forbidden'); }
			},
			_logger: {
				error: _message => { },
				info: _message => { },
				trace: _message => { }
			},
			readSessions: GitHubSessionEngine.prototype['readSessions']
		} satisfies TestGitHubAuthenticationProvider);

		const sessions = await provider.readSessions();
		assert.strictEqual(sessions.length, 0, 'both tokens rejected -> session dropped');
	});

	test('unauthorized error signatures are recognized', () => {
		assert.ok(isUnauthorizedError(new Error('Unauthorized')));
		assert.ok(isUnauthorizedError(new Error('Bad credentials')));
		assert.ok(isUnauthorizedError(new Error('Forbidden')));
		assert.ok(isUnauthorizedError(new Error('401 Unauthorized')));
		assert.ok(!isUnauthorizedError(new Error('network error')));
		assert.ok(!isUnauthorizedError(new Error('')));
	});
});
