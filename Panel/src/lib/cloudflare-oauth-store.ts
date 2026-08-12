import { createHash, randomBytes, timingSafeEqual } from "node:crypto";

export type PendingOAuthSession = {
  id: string;
  state: string;
  claimSecretHash: string;
  installerCallback: string;
  domain: string;
  verifier: string;
  createdAt: number;
  accessToken?: string;
  refreshToken?: string;
  expiresIn?: number;
  scope?: string;
  claimCodeHash?: string;
};

type OAuthStore = Map<string, PendingOAuthSession>;

const globalStore = globalThis as typeof globalThis & { cpnOAuthStore?: OAuthStore };
export const oauthStore = globalStore.cpnOAuthStore ?? new Map<string, PendingOAuthSession>();
globalStore.cpnOAuthStore = oauthStore;

export const randomToken = (bytes = 32) => randomBytes(bytes).toString("base64url");
export const hashToken = (value: string) => createHash("sha256").update(value).digest("hex");

export function tokenMatches(value: string, expectedHash: string): boolean {
  const actual = Buffer.from(hashToken(value), "hex");
  const expected = Buffer.from(expectedHash, "hex");
  return actual.length === expected.length && timingSafeEqual(actual, expected);
}

export function cleanupExpiredSessions(now = Date.now()): void {
  for (const [id, session] of oauthStore) {
    if (now - session.createdAt > 10 * 60 * 1000) oauthStore.delete(id);
  }
}
