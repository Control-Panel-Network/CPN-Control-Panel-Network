import { NextRequest, NextResponse } from "next/server";
import {
  createSessionToken,
  sessionCookieValue,
  verifyCredentials,
} from "../../../lib/auth";

export async function POST(request: NextRequest) {
  const contentType = request.headers.get("content-type") || "";
  let username = "";
  let password = "";

  if (contentType.includes("application/json")) {
    const body = (await request.json().catch(() => ({}))) as {
      username?: string;
      email?: string;
      password?: string;
    };
    username = String(body.username || body.email || "");
    password = String(body.password || "");
  } else {
    const form = await request.formData();
    username = String(form.get("username") || form.get("email") || "");
    password = String(form.get("password") || "");
  }

  if (!username || !password) {
    return NextResponse.json(
      { ok: false, message: "Username and password are required." },
      { status: 400 },
    );
  }

  const boot = await verifyCredentials(username, password);
  if (!boot) {
    return NextResponse.json(
      { ok: false, message: "Invalid username or password." },
      { status: 401 },
    );
  }

  const token = createSessionToken(boot.username);
  const secure = request.nextUrl.protocol === "https:";
  const response = NextResponse.redirect(new URL("/dashboard", request.url), 303);
  response.headers.append("Set-Cookie", sessionCookieValue(token, secure));
  return response;
}
