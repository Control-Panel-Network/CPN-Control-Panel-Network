import { NextResponse } from "next/server";
import { cleanupExpiredSessions, oauthStore, tokenMatches } from "@/lib/cloudflare-oauth-store";

export const runtime = "nodejs";

export async function POST(request: Request) {
  cleanupExpiredSessions();
  const payload = await request.json().catch(() => null);
  const session = typeof payload?.session_id === "string" ? oauthStore.get(payload.session_id) : undefined;
  const valid = session
    && typeof payload.claim_code === "string"
    && typeof payload.claim_secret === "string"
    && typeof session.claimCodeHash === "string"
    && tokenMatches(payload.claim_code, session.claimCodeHash)
    && tokenMatches(payload.claim_secret, session.claimSecretHash);

  if (!valid || !session?.accessToken) {
    return NextResponse.json({ error: "La reclamación OAuth no es válida o expiró" }, { status: 401, headers: { "Cache-Control": "no-store" } });
  }

  oauthStore.delete(session.id);
  return NextResponse.json(
    {
      access_token: session.accessToken,
      refresh_token: session.refreshToken,
      expires_in: session.expiresIn,
      scope: session.scope,
      domain: session.domain,
    },
    { headers: { "Cache-Control": "no-store" } },
  );
}
