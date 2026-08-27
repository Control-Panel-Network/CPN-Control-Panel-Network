import { NextResponse } from "next/server";
import { hasPanelSession } from "@/lib/route-auth";
import { cloudflareStatus } from "@/lib/system-manager";

export async function GET() {
  if (!(await hasPanelSession())) return NextResponse.json({ error: "Sesión no válida" }, { status: 401 });
  try { return NextResponse.json(await cloudflareStatus()); }
  catch (error) { return NextResponse.json({ error: error instanceof Error ? error.message : "No se pudo verificar Cloudflare" }, { status: 500 }); }
}
