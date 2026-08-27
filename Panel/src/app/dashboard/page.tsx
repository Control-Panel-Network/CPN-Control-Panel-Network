import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { PANEL_COOKIE, validSession } from "@/lib/panel-auth";
import { systemInfo } from "@/lib/system-manager";
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
  { label: "Dashboard", icon: Gauge, active: true, href: "/dashboard" },
  { label: "Websites", icon: Globe2, href: "#" },
  { label: "Email", icon: Mail, href: "/email" },
  { label: "Databases", icon: Database, href: "#" },
  { label: "Backups", icon: HardDrive, href: "#" },
];

type SystemInfo = {
  domain: string;
  server: string;
  cpu: { percent: number; cores: number };
  memory: { total_bytes: number; used_bytes: number };
  disk: { total_bytes: number; used_bytes: number };
  services: { web: boolean; postfix: boolean; dovecot: boolean };
};

function gib(bytes: number) { return `${(bytes / 1024 ** 3).toFixed(1)} GB`; }
function percent(used: number, total: number) { return total ? Math.round((used / total) * 100) : 0; }

export default async function DashboardPage() {
  const cookieStore = await cookies();
  if (!validSession(cookieStore.get(PANEL_COOKIE)?.value)) redirect("/");
  const system = await systemInfo() as SystemInfo;
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
              <strong>{system.domain}</strong>
              <span>Panel conectado</span>
            </div>
          </div>

          <nav aria-label="Primary navigation">
            {navigation.map(({ label, icon: Icon, active, href }) => (
              <Link key={label} href={href} className={active ? "active" : undefined}>
                <Icon size={20} strokeWidth={1.8} />
                {label}
              </Link>
            ))}
          </nav>
        </div>

        <div className="sidebar-footer">
          <button aria-label="Notifications"><Bell size={19} /></button>
          <button aria-label="Settings"><Settings size={19} /></button>
          <form action="/api/auth/logout" method="post"><button type="submit" className="logout"><LogOut size={18} /> Log out</button></form>
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
            <h1>Dashboard</h1>
            <p>Monitor your server and manage its services from one place.</p>
          </div>
          <label className="dashboard-search">
            <Search size={18} aria-hidden="true" />
            <span className="sr-only">Search</span>
            <input type="search" placeholder="Search services" />
          </label>
        </div>

        <div className="resource-grid">
          <ResourceGauge label="CPU Usage" value={system.cpu.percent} detail={`${system.cpu.cores} cores`} />
          <ResourceGauge label="RAM Usage" value={percent(system.memory.used_bytes, system.memory.total_bytes)} detail={`${gib(system.memory.used_bytes)} / ${gib(system.memory.total_bytes)}`} />
          <ResourceGauge label="Disk Usage" value={percent(system.disk.used_bytes, system.disk.total_bytes)} detail={`${gib(system.disk.used_bytes)} / ${gib(system.disk.total_bytes)}`} />
        </div>

        <div className="dashboard-lower-grid">
          <article className="status-card">
            <div className="status-card-heading">
              <div>
                <p className="eyebrow">SYSTEM HEALTH</p>
                <h2>{Object.values(system.services).every(Boolean) ? "All services operational" : "A service needs attention"}</h2>
              </div>
              <ShieldCheck size={27} aria-hidden="true" />
            </div>
            <ul>
              <li><span>{system.server}</span><strong>{system.services.web ? "Running" : "Stopped"}</strong></li>
              <li><span>Postfix</span><strong>{system.services.postfix ? "Running" : "Stopped"}</strong></li>
              <li><span>Dovecot</span><strong>{system.services.dovecot ? "Running" : "Stopped"}</strong></li>
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
