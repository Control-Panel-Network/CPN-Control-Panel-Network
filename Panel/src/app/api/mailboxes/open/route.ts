import { NextResponse } from "next/server";
import { hasPanelSession, sameOrigin } from "@/lib/route-auth";
import { mailboxAccess } from "@/lib/system-manager";

export async function POST(request: Request) {
  if (!(await hasPanelSession())) return NextResponse.json({ error: "Sesión no válida" }, { status: 401 });
  if (!sameOrigin(request)) return NextResponse.json({ error: "Origen no permitido" }, { status: 403 });
  try {
    const input = await request.json() as { address?: string };
    return NextResponse.json(await mailboxAccess(input.address ?? ""));
  } catch (error) { return NextResponse.json({ error: error instanceof Error ? error.message : "No se pudo abrir el correo" }, { status: 400 }); }
}
