import { NextRequest, NextResponse } from "next/server";
import { clearSessionCookie } from "../../../lib/auth";

export async function POST(request: NextRequest) {
  const secure = request.nextUrl.protocol === "https:";
  const response = NextResponse.redirect(new URL("/", request.url), 303);
  response.headers.append("Set-Cookie", clearSessionCookie(secure));
  return response;
}

export async function GET(request: NextRequest) {
  return POST(request);
}
