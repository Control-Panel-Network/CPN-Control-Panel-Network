import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { Search, ShieldCheck } from "lucide-react";
import { PanelShell } from "../../components/PanelShell";
import {
  readSessionCookie,
  sessionCookieName,
  verifySessionToken,
} from "../../lib/auth";

type ResourceGaugeProps = {
  label: string;
  value: number;
  detail: string;
};

function ResourceGauge({ label, value, detail }: ResourceGaugeProps) {
  const radius = 42;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference * (1 - value / 100);

  return (
    <article className="resource-card">
      <h2>{label}</h2>
      <div className="gauge" role="img" aria-label={`${label}: ${value}%`}>
        <svg viewBox="0 0 100 100" aria-hidden="true">
          <circle className="gauge-track" cx="50" cy="50" r={radius} />
          <circle
            className="gauge-value"
            cx="50"
            cy="50"
            r={radius}
            strokeDasharray={circumference}
            strokeDashoffset={offset}
          />
        </svg>
        <div className="gauge-copy">
          <strong>{value}%</strong>
          <span>{detail}</span>
        </div>
      </div>
    </article>
  );
}

type DashboardProps = {
  searchParams?: Promise<{ preview?: string }> | { preview?: string };
};

export default async function DashboardPage({ searchParams }: DashboardProps) {
  const params = await Promise.resolve(searchParams ?? {});
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
  const previewOnly = params.preview === "1" && !sessionUser;

  if (!sessionUser && !previewOnly) {
    redirect("/?notice=auth-required");
  }

  const username = sessionUser || "preview";

  return (
    <PanelShell
      username={username}
      active="dashboard"
      signedInLabel={sessionUser ? "Signed in" : "Preview mode"}
    >
      <div className="dashboard-heading">
        <div>
          <p className="eyebrow">SERVER OVERVIEW</p>
          <h1>{sessionUser ? "Dashboard" : "Dashboard preview"}</h1>
          <p>
            {sessionUser
              ? `Signed in as ${sessionUser}.`
              : "Preview only. Sign in with POST /api/login for a real session."}
          </p>
        </div>
        <label className="dashboard-search">
          <Search size={18} aria-hidden="true" />
          <span className="sr-only">Search</span>
          <input type="search" placeholder="Search services" />
        </label>
      </div>
      <div className="resource-grid">
        <ResourceGauge label="CPU Usage" value={45} detail="4 cores" />
        <ResourceGauge label="RAM Usage" value={72} detail="11.5 / 16 GB" />
        <ResourceGauge label="Disk Usage" value={28} detail="140 / 500 GB" />
      </div>
      <div className="dashboard-lower-grid">
        <article className="status-card">
          <div className="status-card-heading">
            <div>
              <p className="eyebrow">SYSTEM HEALTH</p>
              <h2>All services operational</h2>
            </div>
            <ShieldCheck size={27} aria-hidden="true" />
          </div>
          <ul>
            <li>
              <span>Nginx</span>
              <strong>Running</strong>
            </li>
            <li>
              <span>MariaDB</span>
              <strong>Running</strong>
            </li>
            <li>
              <span>Mail service</span>
              <strong>Running</strong>
            </li>
          </ul>
        </article>
        <article className="activity-card">
          <p className="eyebrow">RECENT ACTIVITY</p>
          <h2>Latest changes</h2>
          <div>
            <span>SSL certificate renewed</span>
            <time>12 min ago</time>
          </div>
          <div>
            <span>Automated backup completed</span>
            <time>2 hr ago</time>
          </div>
          <div>
            <span>System packages updated</span>
            <time>Yesterday</time>
          </div>
        </article>
      </div>
    </PanelShell>
  );
}
