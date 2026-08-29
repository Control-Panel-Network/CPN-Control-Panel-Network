import "server-only";
import { createDecipheriv, randomBytes } from "node:crypto";
import { execFile, spawn } from "node:child_process";
import { promisify } from "node:util";
import { chmod, mkdir, readFile, rename, rm, statfs, writeFile } from "node:fs/promises";
import { cpus, loadavg, totalmem, freemem } from "node:os";
import path from "node:path";

const execute = promisify(execFile);
const CONFIG_PATH = "/etc/cpn/install.json";
const MAILBOXES_PATH = "/var/lib/cpn/mailboxes.json";
const DOVECOT_USERS = "/etc/dovecot/cpn-users";
const POSTFIX_DOMAINS = "/etc/postfix/cpn-domains";
const POSTFIX_MAILBOXES = "/etc/postfix/cpn-mailboxes";
const KEY_PATH = "/etc/cpn/secret.key";
const CLOUDFLARE_PATH = "/var/lib/cpn/secrets/cloudflare.enc";
const MAIL_MASTER_PATH = "/var/lib/cpn/secrets/mail-master.enc";
const AAD = Buffer.from("cpn-secret-store-v1");

export type PanelConfig = {
  domain: string;
  server: "nginx" | "caddy" | "openlitespeed";
  webmail: "snappymail" | "rainloop" | "roundcube" | "thunderbird";
  panel_url: string;
  webmail_url: string;
};

export type Mailbox = { address: string; created_at: number };

async function privateWrite(file: string, contents: string | Buffer) {
  await mkdir(path.dirname(file), { recursive: true, mode: 0o700 });
  const temporary = `${file}.${process.pid}.tmp`;
  await writeFile(temporary, contents, { mode: 0o600 });
  await rename(temporary, file);
  await chmod(file, 0o600);
}

export async function panelConfig(): Promise<PanelConfig> {
  return JSON.parse(await readFile(CONFIG_PATH, "utf8")) as PanelConfig;
}

export async function mailboxes(): Promise<Mailbox[]> {
  try { return JSON.parse(await readFile(MAILBOXES_PATH, "utf8")) as Mailbox[]; }
  catch { return []; }
}

function validLocalPart(value: string) {
  return value.length > 0 && value.length <= 64 && !value.startsWith(".") && !value.endsWith(".")
    && !value.includes("..") && /^[a-zA-Z0-9._-]+$/.test(value);
}

function mailboxDirectory(domain: string, localPart: string) {
  if (!/^(?=.{1,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/i.test(domain)) {
    throw new Error("El dominio configurado no es válido");
  }
  const root = path.resolve("/var/vmail", domain);
  const directory = path.resolve(root, localPart);
  if (path.dirname(directory) !== root) throw new Error("Ruta de buzón no válida");
  return directory;
}

async function sha512Crypt(password: string) {
  return new Promise<string>((resolve, reject) => {
    const child = spawn("openssl", ["passwd", "-6", "-stdin"], { stdio: ["pipe", "pipe", "pipe"] });
    let output = "";
    let error = "";
    child.stdout.on("data", (chunk) => { output += chunk; });
    child.stderr.on("data", (chunk) => { error += chunk; });
    child.on("error", reject);
    child.on("close", (code) => code === 0 && output.trim().startsWith("$6$") ? resolve(output.trim()) : reject(new Error(error || "No se pudo proteger la contraseña")));
    child.stdin.end(`${password}\n`);
  });
}

async function rebuildMailMaps(items: Mailbox[], domain: string) {
  await privateWrite(POSTFIX_DOMAINS, `${domain} OK\n`);
  await privateWrite(POSTFIX_MAILBOXES, items.map(({ address }) => `${address} ${domain}/${address.split("@")[0]}/Maildir/`).join("\n") + (items.length ? "\n" : ""));
  await execute("postmap", [POSTFIX_DOMAINS]);
  await execute("postmap", [POSTFIX_MAILBOXES]);
  await execute("systemctl", ["reload", "postfix", "dovecot"]);
}

async function writeDovecotUsers(contents: string) {
  await privateWrite(DOVECOT_USERS, contents);
  await chmod(DOVECOT_USERS, 0o640);
  await execute("chown", ["root:dovecot", DOVECOT_USERS]);
}

export async function createMailbox(localValue: string, password: string) {
  const localPart = localValue.trim().toLowerCase();
  if (!validLocalPart(localPart) || password.length < 12 || password.length > 256) throw new Error("Usa un nombre válido y una contraseña de al menos 12 caracteres");
  const config = await panelConfig();
  const address = `${localPart}@${config.domain}`;
  const items = await mailboxes();
  if (items.some((item) => item.address === address)) throw new Error("Ese buzón ya existe");
  const hash = await sha512Crypt(password);
  const currentUsers = await readFile(DOVECOT_USERS, "utf8").catch(() => "");
  await writeDovecotUsers(`${currentUsers}${address}:{SHA512-CRYPT}${hash}\n`);
  const mailbox = { address, created_at: Math.floor(Date.now() / 1000) };
  items.push(mailbox);
  await privateWrite(MAILBOXES_PATH, `${JSON.stringify(items, null, 2)}\n`);
  await rebuildMailMaps(items, config.domain);
  const directory = mailboxDirectory(config.domain, localPart);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  await execute("chown", ["-R", "vmail:vmail", directory]);
  await execute("doveadm", ["auth", "test", address, password]);
  return mailbox;
}

export async function deleteMailbox(addressValue: string) {
  const config = await panelConfig();
  const address = addressValue.trim().toLowerCase();
  const suffix = `@${config.domain}`;
  const localPart = address.endsWith(suffix) ? address.slice(0, -suffix.length) : "";
  if (!validLocalPart(localPart)) throw new Error("El buzón no pertenece a este dominio");
  const items = await mailboxes();
  if (!items.some((item) => item.address === address)) throw new Error("El buzón no existe");
  const remaining = items.filter((item) => item.address !== address);
  const users = (await readFile(DOVECOT_USERS, "utf8").catch(() => "")).split("\n").filter((line) => line && !line.startsWith(`${address}:`)).join("\n");
  await writeDovecotUsers(users ? `${users}\n` : "");
  await privateWrite(MAILBOXES_PATH, `${JSON.stringify(remaining, null, 2)}\n`);
  await rebuildMailMaps(remaining, config.domain);
  const directory = mailboxDirectory(config.domain, localPart);
  await rm(directory, { recursive: true, force: true });
}

type Envelope = { version: number; nonce: string; ciphertext: string };

async function decryptSecret<T>(file: string): Promise<T> {
  const key = await readFile(KEY_PATH);
  if (key.length !== 32) throw new Error("La clave local de CPN es inválida");
  const envelope = JSON.parse(await readFile(file, "utf8")) as Envelope;
  const nonce = Buffer.from(envelope.nonce, "base64");
  const sealed = Buffer.from(envelope.ciphertext, "base64");
  const tag = sealed.subarray(sealed.length - 16);
  const ciphertext = sealed.subarray(0, sealed.length - 16);
  const decipher = createDecipheriv("chacha20-poly1305", key, nonce, { authTagLength: 16 });
  decipher.setAAD(AAD, { plaintextLength: ciphertext.length });
  decipher.setAuthTag(tag);
  return JSON.parse(Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString("utf8")) as T;
}

type CloudflareAuthorization = { access_token: string; zone_id: string; zone_name: string; scope?: string };

export async function cloudflareStatus() {
  try {
    const authorization = await decryptSecret<CloudflareAuthorization>(CLOUDFLARE_PATH);
    const response = await fetch(`https://api.cloudflare.com/client/v4/zones/${authorization.zone_id}`, { headers: { Authorization: `Bearer ${authorization.access_token}` }, cache: "no-store" });
    return { connected: true, valid: response.ok, zone: authorization.zone_name, scope: authorization.scope };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return { connected: false, valid: false };
    throw error;
  }
}

const oneTimeLogins = new Map<string, { address: string; expires: number }>();

export async function mailboxAccess(address: string) {
  const items = await mailboxes();
  if (!items.some((item) => item.address === address)) throw new Error("El buzón no existe");
  const config = await panelConfig();
  if (config.webmail !== "roundcube") return { url: config.webmail_url, automatic: false };
  const code = randomBytes(36).toString("base64url");
  oneTimeLogins.set(code, { address, expires: Date.now() + 60_000 });
  return { url: `${config.webmail_url}/?_cpn_sso=${code}`, automatic: true };
}

export async function redeemMailboxAccess(code: string) {
  const login = oneTimeLogins.get(code);
  oneTimeLogins.delete(code);
  if (!login || login.expires < Date.now()) throw new Error("El acceso ya no es válido");
  const password = await decryptSecret<string>(MAIL_MASTER_PATH);
  return { username: `${login.address}*cpn-master`, password };
}

async function active(service: string) {
  try { await execute("systemctl", ["is-active", "--quiet", service]); return true; }
  catch { return false; }
}

export async function systemInfo() {
  const config = await panelConfig();
  const disk = await statfs("/");
  const diskTotal = Number(disk.blocks * disk.bsize);
  const diskAvailable = Number(disk.bavail * disk.bsize);
  const cores = cpus().length || 1;
  const webService = config.server === "openlitespeed" ? "lshttpd" : config.server;
  return {
    ...config,
    cpu: { percent: Math.min(100, Math.round((loadavg()[0] / cores) * 100)), cores },
    memory: { total_bytes: totalmem(), used_bytes: totalmem() - freemem() },
    disk: { total_bytes: diskTotal, used_bytes: diskTotal - diskAvailable },
    services: { web: await active(webService), postfix: await active("postfix"), dovecot: await active("dovecot") },
  };
}
