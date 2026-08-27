import { NextResponse } from "next/server";
import { PANEL_COOKIE } from "@/lib/panel-auth";
import { publicUrl } from "@/lib/route-auth";

export async function POST(request: Request) {
  const response = NextResponse.redirect(publicUrl(request, "/"), 303);
  const secure = request.headers.get("x-forwarded-proto") === "https" || new URL(request.url).protocol === "https:";
  response.cookies.set(PANEL_COOKIE, "", { httpOnly: true, secure, sameSite: "strict", path: "/", maxAge: 0 });
  return response;
}
