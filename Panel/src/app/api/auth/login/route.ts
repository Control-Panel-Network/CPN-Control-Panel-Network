import { NextResponse } from "next/server";
import { createSession, PANEL_COOKIE, validCredentials } from "@/lib/panel-auth";
import { publicUrl } from "@/lib/route-auth";

const attempts = new Map<string, { count: number; resetAt: number }>();

export async function POST(request: Request) {
  const url = new URL(request.url);
  const origin = request.headers.get("origin");
  if (origin && new URL(origin).host !== url.host) return NextResponse.json({ error: "Origen no permitido" }, { status: 403 });
  const client = request.headers.get("cf-connecting-ip") ?? request.headers.get("x-forwarded-for")?.split(",")[0] ?? "unknown";
  const now = Date.now();
  const record = attempts.get(client);
  if (record && record.resetAt > now && record.count >= 5) return NextResponse.json({ error: "Demasiados intentos. Espera 15 minutos." }, { status: 429 });
  const form = await request.formData();
  const email = String(form.get("email") ?? "");
  const password = String(form.get("password") ?? "");
  let valid = false;
  try { valid = await validCredentials(email, password); } catch { /* Fail closed when unconfigured. */ }
  if (!valid) {
    attempts.set(client, { count: record && record.resetAt > now ? record.count + 1 : 1, resetAt: now + 15 * 60_000 });
    return NextResponse.redirect(publicUrl(request, "/?error=invalid"), 303);
  }
  attempts.delete(client);
  const response = NextResponse.redirect(publicUrl(request, "/dashboard"), 303);
  const secure = request.headers.get("x-forwarded-proto") === "https" || new URL(request.url).protocol === "https:";
  response.cookies.set(PANEL_COOKIE, createSession(email), { httpOnly: true, secure, sameSite: "strict", path: "/", maxAge: 8 * 60 * 60 });
  return response;
}
