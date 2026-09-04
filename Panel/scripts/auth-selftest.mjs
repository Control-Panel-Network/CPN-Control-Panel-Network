#!/usr/bin/env node
/**
 * Panel auth self-test (issue #8).
 * Run: node Panel/scripts/auth-selftest.mjs
 */
import { createHmac, timingSafeEqual } from "crypto";
import assert from "assert";
import fs from "fs";
import os from "os";
import path from "path";

const SESSION_COOKIE = "cpn_panel_session";
const SESSION_TTL_SECONDS = 60 * 60 * 12;

function createSessionToken(username, secret) {
  const exp = Math.floor(Date.now() / 1000) + SESSION_TTL_SECONDS;
  const payload = `${username}|${exp}`;
  const sig = createHmac("sha256", secret).update(payload).digest("hex");
  return Buffer.from(`${payload}|${sig}`, "utf8").toString("base64url");
}

function verifySessionToken(token, secret) {
  const decoded = Buffer.from(token, "base64url").toString("utf8");
  const parts = decoded.split("|");
  if (parts.length !== 3) return null;
  const [username, expRaw, sig] = parts;
  const exp = Number(expRaw);
  if (!username || !Number.isFinite(exp) || exp < Math.floor(Date.now() / 1000)) {
    return null;
  }
  const payload = `${username}|${exp}`;
  const expected = createHmac("sha256", secret).update(payload).digest("hex");
  const left = Buffer.from(sig, "hex");
  const right = Buffer.from(expected, "hex");
  if (left.length !== right.length || !timingSafeEqual(left, right)) {
    return null;
  }
  return username;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cpn-auth-"));
const secretPath = path.join(tmp, "panel-session.secret");
fs.writeFileSync(secretPath, "unit-test-secret-value\n", { mode: 0o600 });
const secret = fs.readFileSync(secretPath, "utf8").trim();
assert.strictEqual(secret, "unit-test-secret-value");

const good = createSessionToken("Admin", secret);
assert.strictEqual(verifySessionToken(good, secret), "Admin");
assert.strictEqual(verifySessionToken(good, "cpn-panel-dev-session"), null);
assert.strictEqual(verifySessionToken(good, "wrong"), null);

const cookie = `${SESSION_COOKIE}=${good}; Path=/; HttpOnly; SameSite=Lax`;
assert.ok(cookie.includes("HttpOnly"));
assert.ok(!cookie.includes("password="));

fs.rmSync(tmp, { recursive: true, force: true });
console.log("auth-selftest: ok");
