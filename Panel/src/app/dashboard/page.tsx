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

/** Traffic-light stroke: green < 60%, orange 60-84%, red >= 85%. */
function gaugeStrokeForUsage(percent: number): string {
  if (percent >= 85) return "#d92d20";
  if (percent >= 60) return "#f79009";
  return "#12b76a";
}

function ResourceGauge({ label, value, detail }: ResourceGaugeProps) {
  const radius = 42;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference * (1 - value / 100);
  const stroke = gaugeStrokeForUsage(value);

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
            stroke={stroke}
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
              <h2>Service status (live on host Panel)</h2>
            </div>
            <ShieldCheck size={27} aria-hidden="true" />
          </div>
          <ul>
            <li>
              <span>Web server</span>
              <strong className="warn">See host Panel</strong>
            </li>
            <li>
              <span>MariaDB</span>
              <strong className="warn">See host Panel</strong>
            </li>
            <li>
              <span>Mail service</span>
              <strong className="warn">See host Panel</strong>
            </li>
          </ul>
          <p className="muted" style={{ marginTop: 14 }}>
            Preview build does not probe systemd. The host Panel uses the same
            detection as the Databases page (no fake Running states).
          </p>
        </article>
        <article className="activity-card">
          <p className="eyebrow">RECENT ACTIVITY</p>
          <h2>Latest changes</h2>
          <p className="empty-state">No recent panel activity to show yet.</p>
        </article>
      </div>
    </PanelShell>
  );
}
