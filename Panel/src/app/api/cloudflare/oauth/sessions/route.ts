import { NextResponse } from "next/server";
import { oauthConfiguration, OAUTH_CALLBACK_URL, pkceChallenge, validatedDomain, validatedInstallerCallback } from "@/lib/cloudflare-oauth";
import { cleanupExpiredSessions, hashToken, oauthStore, randomToken } from "@/lib/cloudflare-oauth-store";

export const runtime = "nodejs";

export async function POST(request: Request) {
  try {
    cleanupExpiredSessions();
    const payload = await request.json();
    const callback = validatedInstallerCallback(payload.installer_callback);
    const domain = validatedDomain(payload.domain);
    const claimSecret = randomToken();
    const id = randomToken(18);
    const state = randomToken();
    const verifier = randomToken(48);
    const { clientId, scopes } = oauthConfiguration();

    oauthStore.set(id, {
      id,
      state,
      claimSecretHash: hashToken(claimSecret),
      installerCallback: callback.toString(),
      domain,
      verifier,
      createdAt: Date.now(),
    });

    const authorize = new URL("https://dash.cloudflare.com/oauth2/auth");
    authorize.searchParams.set("client_id", clientId);
    authorize.searchParams.set("redirect_uri", OAUTH_CALLBACK_URL);
    authorize.searchParams.set("response_type", "code");
    authorize.searchParams.set("scope", scopes);
    authorize.searchParams.set("state", state);
    authorize.searchParams.set("code_challenge", pkceChallenge(verifier));
    authorize.searchParams.set("code_challenge_method", "S256");

    return NextResponse.json(
      { session_id: id, claim_secret: claimSecret, authorization_url: authorize.toString() },
      { headers: { "Cache-Control": "no-store" } },
    );
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : "No se pudo iniciar OAuth" },
      { status: 400, headers: { "Cache-Control": "no-store" } },
    );
  }
}
