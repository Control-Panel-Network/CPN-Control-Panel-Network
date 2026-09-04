import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { PanelShell } from "./PanelShell";
import {
  readSessionCookie,
  sessionCookieName,
  verifySessionToken,
} from "../lib/auth";

type SectionPageProps = {
  title: string;
  active: string;
  blurb: string;
};

async function requireSession(): Promise<string> {
  const jar = await cookies();
  const raw =
    jar.get(sessionCookieName())?.value ||
    readSessionCookie(
      jar
        .getAll()
        .map((item) => `${item.name}=${item.value}`)
        .join("; "),
    );
  const sessionUser = raw ? verifySessionToken(raw) : null;
  if (!sessionUser) {
    redirect("/?notice=auth-required");
  }
  return sessionUser;
}

export async function SectionPage({ title, active, blurb }: SectionPageProps) {
  const username = await requireSession();
  return (
    <PanelShell username={username} active={active} signedInLabel="Signed in">
      <div className="dashboard-heading">
        <div>
          <p className="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>
      <article className="section-card">
        <h2>{title}</h2>
        <p>
          This section uses the same responsive shell as the dashboard.
          Management tools for {title.toLowerCase()} will land here in a later
          release.
        </p>
      </article>
    </PanelShell>
  );
}
