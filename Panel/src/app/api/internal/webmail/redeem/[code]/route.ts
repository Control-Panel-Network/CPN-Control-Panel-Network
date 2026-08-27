import { NextResponse } from "next/server";
import { timingSafeEqual } from "node:crypto";
import { redeemMailboxAccess } from "@/lib/system-manager";

function same(left: string, right: string) {
  const a = Buffer.from(left);
  const b = Buffer.from(right);
  return a.length === b.length && timingSafeEqual(a, b);
}

export async function POST(request: Request, context: RouteContext<"/api/internal/webmail/redeem/[code]">) {
  const expected = process.env.CPN_PANEL_WEBMAIL_TOKEN;
  const supplied = request.headers.get("authorization")?.replace(/^Bearer /, "");
  if (!expected || !supplied || !same(supplied, expected)) return NextResponse.json({ error: "No autorizado" }, { status: 401 });
  try {
    const { code } = await context.params;
    return NextResponse.json(await redeemMailboxAccess(code));
  } catch (error) { return NextResponse.json({ error: error instanceof Error ? error.message : "Acceso inválido" }, { status: 410 }); }
}
