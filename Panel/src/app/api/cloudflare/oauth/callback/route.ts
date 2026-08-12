import { NextResponse } from "next/server";
import { CLOUDFLARE_TOKEN_URL, oauthConfiguration, OAUTH_CALLBACK_URL } from "@/lib/cloudflare-oauth";
import { cleanupExpiredSessions, hashToken, oauthStore, randomToken } from "@/lib/cloudflare-oauth-store";

export const runtime = "nodejs";

type TokenResponse = {
  access_token?: string;
  refresh_token?: string;
  expires_in?: number;
  scope?: string;
  error_description?: string;
};

export async function GET(request: Request) {
  cleanupExpiredSessions();
  const current = new URL(request.url);
  const state = current.searchParams.get("state");
  const code = current.searchParams.get("code");
  const oauthError = current.searchParams.get("error_description") ?? current.searchParams.get("error");
  const session = [...oauthStore.values()].find((candidate) => candidate.state === state);
  if (!session) return NextResponse.json({ error: "La sesión OAuth expiró o no es válida" }, { status: 400 });

  const redirect = new URL(session.installerCallback);
  redirect.searchParams.set("session", session.id);
  if (oauthError || !code) {
    oauthStore.delete(session.id);
    redirect.searchParams.set("oauth_error", oauthError ?? "Cloudflare no devolvió un código de autorización");
    return NextResponse.redirect(redirect);
  }

  try {
    const { clientId, clientSecret } = oauthConfiguration();
    const response = await fetch(CLOUDFLARE_TOKEN_URL, {
      method: "POST",
      headers: {
        Authorization: `Basic ${Buffer.from(`${clientId}:${clientSecret}`).toString("base64")}`,
        "Content-Type": "application/x-www-form-urlencoded",
        Accept: "application/json",
      },
      body: new URLSearchParams({
        grant_type: "authorization_code",
        code,
        redirect_uri: OAUTH_CALLBACK_URL,
        code_verifier: session.verifier,
      }),
      cache: "no-store",
    });
    const token = (await response.json()) as TokenResponse;
    if (!response.ok || !token.access_token) throw new Error(token.error_description ?? "Cloudflare rechazó el intercambio del código");

    const claimCode = randomToken();
    session.accessToken = token.access_token;
    session.refreshToken = token.refresh_token;
    session.expiresIn = token.expires_in;
    session.scope = token.scope;
    session.claimCodeHash = hashToken(claimCode);
    redirect.searchParams.set("claim", claimCode);
    return NextResponse.redirect(redirect);
  } catch (error) {
    oauthStore.delete(session.id);
    redirect.searchParams.set("oauth_error", error instanceof Error ? error.message : "OAuth falló");
    return NextResponse.redirect(redirect);
  }
}
