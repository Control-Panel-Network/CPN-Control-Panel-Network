import "server-only";
import { cookies } from "next/headers";
import { PANEL_COOKIE, validSession } from "@/lib/panel-auth";

export async function hasPanelSession() {
  const store = await cookies();
  return validSession(store.get(PANEL_COOKIE)?.value);
}

export function sameOrigin(request: Request) {
  const origin = request.headers.get("origin");
  if (!origin) return true;
  try {
    const expected = request.headers.get("x-forwarded-host") ?? request.headers.get("host") ?? new URL(request.url).host;
    return new URL(origin).host === expected;
  } catch {
    return false;
  }
}

export function publicUrl(request: Request, pathname: string) {
  const current = new URL(request.url);
  const host = request.headers.get("x-forwarded-host") ?? request.headers.get("host") ?? current.host;
  const protocol = request.headers.get("x-forwarded-proto") ?? current.protocol.replace(":", "");
  return new URL(pathname, `${protocol}://${host}`);
}
