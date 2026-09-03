import { NextResponse } from "next/server";

export async function POST() {
  return NextResponse.json(
    {
      ok: false,
      message:
        "Password reset mail is not connected yet. The recovery email collected during install is the intended destination once SMTP is configured.",
    },
    { status: 501 },
  );
}
