import { createHash } from "node:crypto";

export const CLOUDFLARE_AUTHORIZE_URL = "https://dash.cloudflare.com/oauth2/auth";
export const CLOUDFLARE_TOKEN_URL = "https://dash.cloudflare.com/oauth2/token";
export const OAUTH_CALLBACK_URL = "https://panel.discord-bot-network.com/api/cloudflare/oauth/callback";

export function oauthConfiguration() {
  const clientId = process.env.CLOUDFLARE_OAUTH_CLIENT_ID;
  const clientSecret = process.env.CLOUDFLARE_OAUTH_CLIENT_SECRET;
  const scopes = process.env.CLOUDFLARE_OAUTH_SCOPES;
  if (!clientId || !clientSecret || !scopes) {
    throw new Error("El puente OAuth de Cloudflare aún no tiene credenciales configuradas");
  }
  return { clientId, clientSecret, scopes };
}

export function pkceChallenge(verifier: string): string {
  return createHash("sha256").update(verifier).digest("base64url");
}

export function validatedInstallerCallback(input: unknown): URL {
  if (typeof input !== "string") throw new Error("Falta la URL de retorno del instalador");
  const callback = new URL(input);
  if (!["http:", "https:"].includes(callback.protocol)) throw new Error("Protocolo de retorno no permitido");
  if (callback.username || callback.password || callback.hash) throw new Error("URL de retorno no permitida");
  if (callback.pathname !== "/api/dns/cloudflare/callback") throw new Error("Ruta de retorno no permitida");
  if (callback.port && callback.port !== "8787") throw new Error("Puerto de retorno no permitido");
  return callback;
}

export function validatedDomain(input: unknown): string {
  if (typeof input !== "string") throw new Error("Falta el dominio");
  const domain = input.trim().toLowerCase().replace(/\.$/, "");
  if (domain.length > 253 || !domain.includes(".")) throw new Error("Dominio no válido");
  for (const label of domain.split(".")) {
    if (!/^(?!-)[a-z0-9-]{1,63}(?<!-)$/.test(label)) throw new Error("Dominio no válido");
  }
  return domain;
}
