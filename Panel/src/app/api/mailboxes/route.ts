import { NextResponse } from "next/server";
import { hasPanelSession, sameOrigin } from "@/lib/route-auth";
import { createMailbox, deleteMailbox, mailboxes } from "@/lib/system-manager";

async function authorizedMutation(request: Request) {
  if (!(await hasPanelSession())) return NextResponse.json({ error: "Sesión no válida" }, { status: 401 });
  if (!sameOrigin(request)) return NextResponse.json({ error: "Origen no permitido" }, { status: 403 });
  return null;
}

export async function GET() {
  if (!(await hasPanelSession())) return NextResponse.json({ error: "Sesión no válida" }, { status: 401 });
  return NextResponse.json(await mailboxes());
}

export async function POST(request: Request) {
  const denied = await authorizedMutation(request);
  if (denied) return denied;
  try {
    const input = await request.json() as { local_part?: string; password?: string };
    return NextResponse.json(await createMailbox(input.local_part ?? "", input.password ?? ""), { status: 201 });
  } catch (error) { return NextResponse.json({ error: error instanceof Error ? error.message : "No se pudo crear el buzón" }, { status: 400 }); }
}

export async function DELETE(request: Request) {
  const denied = await authorizedMutation(request);
  if (denied) return denied;
  try {
    const input = await request.json() as { address?: string };
    await deleteMailbox(input.address ?? "");
    return new NextResponse(null, { status: 204 });
  } catch (error) { return NextResponse.json({ error: error instanceof Error ? error.message : "No se pudo eliminar el buzón" }, { status: 400 }); }
}
