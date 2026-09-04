import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import {
  Bell,
  Database,
  Gauge,
  Globe2,
  HardDrive,
  LogOut,
  Mail,
  Menu,
  Network,
  Search,
  Server,
  Settings,
  ShieldCheck,
} from "lucide-react";
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

const navigation = [
  { label: "Dashboard", icon: Gauge, active: true },
  { label: "Websites", icon: Globe2 },
  { label: "Email", icon: Mail },
  { label: "Databases", icon: Database },
  { label: "Backups", icon: HardDrive },
];

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

  return (
    <main className="panel-layout">
      <aside className="sidebar">
        <div>
          <Link href="/dashboard" className="panel-brand">
            <Server size={23} strokeWidth={1.9} />
            <span>NT&amp;DBN Panel</span>
          </Link>
          <div className="server-summary">
            <Network size={20} aria-hidden="true" />
            <div>
              <strong>{sessionUser || "preview"}</strong>
              <span>{sessionUser ? "Signed in" : "Preview mode"}</span>
            </div>
          </div>
          <nav aria-label="Primary navigation">
            {navigation.map(({ label, icon: Icon, active }) => (
              <a key={label} href="#" className={active ? "active" : undefined}>
                <Icon size={20} strokeWidth={1.8} />
                {label}
              </a>
            ))}
          </nav>
        </div>
        <div className="sidebar-footer">
          <button aria-label="Notifications"><Bell size={19} /></button>
          <button aria-label="Settings"><Settings size={19} /></button>
          <Link href="/api/logout" className="logout"><LogOut size={18} /> Log out</Link>
        </div>
      </aside>
      <section className="panel-main">
        <header className="mobile-header">
          <button aria-label="Open menu"><Menu size={22} /></button>
          <strong>NT&amp;DBN Panel</strong>
          <button aria-label="Notifications"><Bell size={21} /></button>
        </header>
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
              <li><span>Nginx</span><strong>Running</strong></li>
              <li><span>MariaDB</span><strong>Running</strong></li>
              <li><span>Mail service</span><strong>Running</strong></li>
            </ul>
          </article>
          <article className="activity-card">
            <p className="eyebrow">RECENT ACTIVITY</p>
            <h2>Latest changes</h2>
            <div><span>SSL certificate renewed</span><time>12 min ago</time></div>
            <div><span>Automated backup completed</span><time>2 hr ago</time></div>
            <div><span>System packages updated</span><time>Yesterday</time></div>
          </article>
        </div>
      </section>
    </main>
  );
}
