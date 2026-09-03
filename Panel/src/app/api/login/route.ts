import { NextResponse } from "next/server";

export async function POST() {
  // Placeholder endpoint so the login form can POST without leaking credentials
  // into the URL. Real session cookies will be added when Panel auth lands.
  return NextResponse.json(
    {
      ok: false,
      message:
        "Authentication backend is not connected yet. Credentials were accepted by POST only and were not stored in the URL.",
    },
    { status: 501 },
  );
}
