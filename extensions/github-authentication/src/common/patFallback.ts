/*---------------------------------------------------------------------------------------------
 *  POLER Studio - Service PAT fallback for the GitHub authentication provider
 *
 *  Stage C.1 of the sovereign Copilot gateway: when the locally stored
 *  OAuth session (state.vscdb) is rejected by GitHub with 401/403, the
 *  provider retries the request with a service Personal Access Token sent
 *  as `Authorization: Bearer <GITHUB_TOKEN>`. The token is resolved from
 *  (in order): the GITHUB_TOKEN environment variable, GH_TOKEN, GITHUB_PAT.
 *--------------------------------------------------------------------------------------------*/

/**
 * Messages GitHub answers 401/403 with, as surfaced by `fetchUserInfo`
 * (either the HTTP status text or the JSON `message` field).
 */
const UNAUTHORIZED_MESSAGES = new Set([
	'Unauthorized',
	'Forbidden',
	'Bad credentials',
	'Requires authentication',
	'Resource not accessible by integration',
	'Personal access token with read:user scope required'
]);

/**
 * Whether a `getUserInfo` failure means the token was rejected (401/403)
 * rather than a plain network/parse failure.
 */
export function isUnauthorizedError(error: { message?: string }): boolean {
	const message = error?.message?.trim();
	if (!message) {
		return false;
	}
	if (UNAUTHORIZED_MESSAGES.has(message)) {
		return true;
	}
	// Copilot API passes through raw status lines such as "401 Unauthorized".
	return /^40[13]\b/.test(message);
}

/**
 * Resolve the service Personal Access Token. Returns `undefined` when no
 * token is configured - in that case callers keep the original behavior.
 */
export function getServicePersonalAccessToken(): string | undefined {
	const fromEnv = process.env.GITHUB_TOKEN || process.env.GH_TOKEN || process.env.GITHUB_PAT;
	if (fromEnv && fromEnv.trim()) {
		return fromEnv.trim();
	}
	return undefined;
}
