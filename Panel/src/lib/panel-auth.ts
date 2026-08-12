import "server-only";
import { createHmac, scrypt as scryptCallback, timingSafeEqual } from "node:crypto";
import { promisify } from "node:util";

const scrypt = promisify(scryptCallback);
export const PANEL_COOKIE = "cpn_panel_session";

function configuration() {
  const email = process.env.CPN_PANEL_ADMIN_EMAIL?.trim().toLowerCase();
  const passwordHash = process.env.CPN_PANEL_ADMIN_PASSWORD_SCRYPT;
  const sessionSecret = process.env.CPN_PANEL_SESSION_SECRET;
  if (!email || !passwordHash || !sessionSecret || sessionSecret.length < 32) throw new Error("La autenticación del Panel no está configurada");
  return { email, passwordHash, sessionSecret };
}

export async function validCredentials(email: string, password: string): Promise<boolean> {
  const config = configuration();
  const [saltHex, expectedHex] = config.passwordHash.split(":");
  if (!saltHex || !expectedHex || !/^[a-f0-9]+$/i.test(saltHex + expectedHex)) return false;
  const expected = Buffer.from(expectedHex, "hex");
  const actual = await scrypt(password, Buffer.from(saltHex, "hex"), expected.length) as Buffer;
  const sameEmail = Buffer.from(email.trim().toLowerCase()).length === Buffer.from(config.email).length
    && timingSafeEqual(Buffer.from(email.trim().toLowerCase()), Buffer.from(config.email));
  return sameEmail && actual.length === expected.length && timingSafeEqual(actual, expected);
}

export function createSession(email: string): string {
  const { sessionSecret } = configuration();
  const expires = Math.floor(Date.now() / 1000) + 8 * 60 * 60;
  const payload = Buffer.from(JSON.stringify({ email: email.toLowerCase(), expires })).toString("base64url");
  const signature = createHmac("sha256", sessionSecret).update(payload).digest("base64url");
  return `${payload}.${signature}`;
}

export function validSession(token: string | undefined): boolean {
  if (!token) return false;
  try {
    const { sessionSecret } = configuration();
    const [payload, supplied] = token.split(".");
    const expected = createHmac("sha256", sessionSecret).update(payload).digest();
    const signature = Buffer.from(supplied, "base64url");
    if (signature.length !== expected.length || !timingSafeEqual(signature, expected)) return false;
    const data = JSON.parse(Buffer.from(payload, "base64url").toString()) as { expires?: number };
    return typeof data.expires === "number" && data.expires > Date.now() / 1000;
  } catch { return false; }
}
