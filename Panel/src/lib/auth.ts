import { createHmac, pbkdf2Sync, timingSafeEqual } from "crypto";
import { promises as fs, readFileSync } from "fs";
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

/**
 * Browser cookies ignore TCP port. Lab panels on 127.0.0.1:2087 vs :2092 must
 * use distinct names (`cpn_panel_session_2092`) so concurrent logins stay independent.
 */
export function sessionCookieNameForPort(port?: number | null): string {
  if (port && port > 0) {
    return `${SESSION_COOKIE}_${port}`;
  }
  return SESSION_COOKIE;
}

/** Parse an explicit port from an HTTP Host header (NAT lab ports). */
export function authCookiePortFromHost(
  host: string | null | undefined,
): number | null {
  const raw = String(host || "").trim();
  if (!raw || raw.length > 253) return null;
  if (raw.startsWith("[")) {
    const end = raw.indexOf("]");
    if (end < 0) return null;
    const after = raw.slice(end + 1);
    if (!after.startsWith(":")) return null;
    const port = Number(after.slice(1));
    return Number.isInteger(port) && port > 0 && port <= 65535 ? port : null;
  }
  if ((raw.match(/:/g) || []).length !== 1) return null;
  const port = Number(raw.split(":")[1]);
  return Number.isInteger(port) && port > 0 && port <= 65535 ? port : null;
}

/** Match Rust `PBKDF2_ITERATIONS`. */
export const PBKDF2_ITERATIONS = 600_000;

function dataDir(): string {
  return process.env.CPN_DATA_DIR || "/var/lib/cpn";
}

export function bootstrapPath(): string {
  return (
    process.env.PANEL_BOOTSTRAP_PATH ||
    path.join(dataDir(), "panel-bootstrap.json")
  );
}

function secretFilePath(): string {
  return path.join(dataDir(), "panel-session.secret");
}

/**
 * Resolve HMAC secret for panel sessions (issue #8).
 * Never fall back to a public hardcoded secret in production.
 */
function sessionSecret(): string {
  const fromEnv = (process.env.CPN_PANEL_SESSION_SECRET || "").trim();
  if (fromEnv) {
    return fromEnv;
  }
  try {
    const fromFile = readFileSync(secretFilePath(), "utf8").trim();
    if (fromFile) {
      return fromFile;
    }
  } catch {
    // Missing file is handled below.
  }
  const fromInstaller = (process.env.CPN_INSTALL_TOKEN || "").trim();
  if (fromInstaller) {
    return fromInstaller;
  }
  if (
    process.env.CPN_ALLOW_DEV_SESSION === "1" ||
    process.env.NODE_ENV === "development" ||
    process.env.NODE_ENV === "test"
  ) {
    return "cpn-panel-dev-session";
  }
  throw new Error(
    "CPN panel session secret missing. Set CPN_PANEL_SESSION_SECRET or create /var/lib/cpn/panel-session.secret (issue #8).",
  );
}

function saltMaterial(saltHex: string): Buffer {
  const trimmed = saltHex.trim();
  if (
    trimmed.length > 0 &&
    trimmed.length % 2 === 0 &&
    /^[0-9a-fA-F]+$/.test(trimmed)
  ) {
    return Buffer.from(trimmed, "hex");
  }
  return Buffer.from(saltHex, "utf8");
}

/** Match Rust `hash_password`: `pbkdf2$<iters>$<hex>`. */
export function hashPassword(password: string, saltHex: string): string {
  const key = pbkdf2Sync(
    password,
    saltMaterial(saltHex),
    PBKDF2_ITERATIONS,
    32,
    "sha256",
  );
  return `pbkdf2$${PBKDF2_ITERATIONS}$${key.toString("hex")}`;
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

export function verifyPassword(
  password: string,
  saltHex: string,
  stored: string,
): boolean {
  const value = stored.trim();
  if (!value.startsWith("pbkdf2$")) {
    // Legacy SHA-256 hashes are not verified in Panel (CodeQL). Reset via `cpn account passwd`.
    return false;
  }
  const rest = value.slice("pbkdf2$".length);
  const sep = rest.indexOf("$");
  if (sep <= 0) return false;
  const iters = Number(rest.slice(0, sep));
  const digestHex = rest.slice(sep + 1);
  if (!Number.isFinite(iters) || iters <= 0 || iters > 5_000_000) {
    return false;
  }
  const key = pbkdf2Sync(password, saltMaterial(saltHex), iters, 32, "sha256");
  return safeEqualHex(key.toString("hex"), digestHex);
}

export async function loadBootstrap(): Promise<PanelBootstrap | null> {
  try {
    const raw = await fs.readFile(
      /*turbopackIgnore: true*/ bootstrapPath(),
      "utf8",
    );
    return JSON.parse(raw) as PanelBootstrap;
  } catch {
    return null;
  }
}

export async function saveBootstrap(boot: PanelBootstrap): Promise<void> {
  const target = bootstrapPath();
  await fs.mkdir(path.dirname(target), { recursive: true });
  await fs.writeFile(target, JSON.stringify(boot, null, 2), {
    encoding: "utf8",
    mode: 0o600,
  });
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
  if (!verifyPassword(password, boot.password_salt, boot.password_hash)) {
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
    if (
      !username ||
      !Number.isFinite(exp) ||
      exp < Math.floor(Date.now() / 1000)
    ) {
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

export function sessionCookieName(port?: number | null): string {
  return sessionCookieNameForPort(port);
}

export function sessionCookieValue(
  token: string,
  secure: boolean,
  port?: number | null,
): string {
  const secureFlag = secure ? "; Secure" : "";
  const name = sessionCookieNameForPort(port);
  return `${name}=${token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${SESSION_TTL_SECONDS}${secureFlag}`;
}

export function clearSessionCookie(
  secure: boolean,
  port?: number | null,
): string {
  const secureFlag = secure ? "; Secure" : "";
  const name = sessionCookieNameForPort(port);
  return `${name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0${secureFlag}`;
}

function readNamedCookie(cookieHeader: string, name: string): string | null {
  const prefix = `${name}=`;
  for (const part of cookieHeader.split(";")) {
    const trimmed = part.trim();
    if (trimmed.startsWith(prefix)) {
      const value = trimmed.slice(prefix.length).trim();
      return value || null;
    }
  }
  return null;
}

export function readSessionCookie(
  cookieHeader: string | null,
  port?: number | null,
): string | null {
  if (!cookieHeader) return null;
  const scoped = sessionCookieNameForPort(port);
  const fromScoped = readNamedCookie(cookieHeader, scoped);
  if (fromScoped) return fromScoped;
  if (port && port > 0) {
    return readNamedCookie(cookieHeader, SESSION_COOKIE);
  }
  return null;
}
