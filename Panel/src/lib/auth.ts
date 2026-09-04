import { createHash, createHmac, timingSafeEqual } from "crypto";
import { promises as fs } from "fs";
import path from "path";

export type PanelBootstrap = {
  schema_version: number;
  username: string;
  recovery_email: string;
  password_hash: string;
  password_salt: string;
  language?: string;
};

const SESSION_COOKIE = "cpn_panel_session";
const SESSION_TTL_SECONDS = 60 * 60 * 12;

function dataDir(): string {
  return process.env.CPN_DATA_DIR || "/var/lib/cpn";
}

export function bootstrapPath(): string {
  return (
    process.env.PANEL_BOOTSTRAP_PATH ||
    path.join(dataDir(), "panel-bootstrap.json")
  );
}

function sessionSecret(): string {
  return (
    process.env.CPN_PANEL_SESSION_SECRET ||
    process.env.CPN_INSTALL_TOKEN ||
    "cpn-panel-dev-session"
  );
}

/** Match Rust `hash_password`: SHA-256(salt_hex + "|" + password) as hex. */
export function hashPassword(password: string, saltHex: string): string {
  return createHash("sha256")
    .update(saltHex, "utf8")
    .update("|", "utf8")
    .update(password, "utf8")
    .digest("hex");
}

function safeEqualHex(a: string, b: string): boolean {
  try {
    const left = Buffer.from(a, "hex");
    const right = Buffer.from(b, "hex");
    if (left.length !== right.length) return false;
    return timingSafeEqual(left, right);
  } catch {
    return false;
  }
}

export async function loadBootstrap(): Promise<PanelBootstrap | null> {
  try {
    const raw = await fs.readFile(/*turbopackIgnore: true*/ bootstrapPath(), "utf8");
    return JSON.parse(raw) as PanelBootstrap;
  } catch {
    return null;
  }
}

export async function verifyCredentials(
  username: string,
  password: string,
): Promise<PanelBootstrap | null> {
  const boot = await loadBootstrap();
  if (!boot) return null;
  if (boot.username.trim().toLowerCase() !== username.trim().toLowerCase()) {
    return null;
  }
  const hashed = hashPassword(password, boot.password_salt);
  if (!safeEqualHex(hashed, boot.password_hash)) {
    return null;
  }
  return boot;
}

export function createSessionToken(username: string): string {
  const exp = Math.floor(Date.now() / 1000) + SESSION_TTL_SECONDS;
  const payload = `${username}|${exp}`;
  const sig = createHmac("sha256", sessionSecret())
    .update(payload)
    .digest("hex");
  return Buffer.from(`${payload}|${sig}`, "utf8").toString("base64url");
}

export function verifySessionToken(token: string): string | null {
  try {
    const decoded = Buffer.from(token, "base64url").toString("utf8");
    const parts = decoded.split("|");
    if (parts.length !== 3) return null;
    const [username, expRaw, sig] = parts;
    const exp = Number(expRaw);
    if (!username || !Number.isFinite(exp) || exp < Math.floor(Date.now() / 1000)) {
      return null;
    }
    const payload = `${username}|${exp}`;
    const expected = createHmac("sha256", sessionSecret())
      .update(payload)
      .digest("hex");
    const left = Buffer.from(sig, "hex");
    const right = Buffer.from(expected, "hex");
    if (left.length !== right.length || !timingSafeEqual(left, right)) {
      return null;
    }
    return username;
  } catch {
    return null;
  }
}

export function sessionCookieName(): string {
  return SESSION_COOKIE;
}

export function sessionCookieValue(token: string, secure: boolean): string {
  const secureFlag = secure ? "; Secure" : "";
  return `${SESSION_COOKIE}=${token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=${SESSION_TTL_SECONDS}${secureFlag}`;
}

export function clearSessionCookie(secure: boolean): string {
  const secureFlag = secure ? "; Secure" : "";
  return `${SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0${secureFlag}`;
}

export function readSessionCookie(
  cookieHeader: string | null,
): string | null {
  if (!cookieHeader) return null;
  for (const part of cookieHeader.split(";")) {
    const trimmed = part.trim();
    if (trimmed.startsWith(`${SESSION_COOKIE}=`)) {
      const value = trimmed.slice(SESSION_COOKIE.length + 1).trim();
      return value || null;
    }
  }
  return null;
}
