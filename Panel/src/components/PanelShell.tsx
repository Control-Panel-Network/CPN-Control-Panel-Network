"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import {
  AppWindow,
  Database,
  Gauge,
  Globe2,
  HardDrive,
  LogOut,
  Mail,
  Menu,
  Network,
  Puzzle,
  Server,
  Settings,
  Shield,
  Users,
  X,
} from "lucide-react";

type NavItem = { label: string; href: string; icon: typeof Gauge; id: string };

const hosting: NavItem[] = [
  { label: "Dashboard", href: "/dashboard", icon: Gauge, id: "dashboard" },
  { label: "Websites", href: "/websites", icon: Globe2, id: "websites" },
  { label: "Email", href: "/email", icon: Mail, id: "email" },
  { label: "Databases & FTP", href: "/databases", icon: Database, id: "databases" },
  { label: "Backups", href: "/backups", icon: HardDrive, id: "backups" },
  { label: "Apps", href: "/apps", icon: AppWindow, id: "apps" },
  { label: "Plugins", href: "/plugins", icon: Puzzle, id: "plugins" },
];

const account: NavItem[] = [
  { label: "Users & Plans", href: "/account/users", icon: Users, id: "users" },
];

const administration: NavItem[] = [
  { label: "Server", href: "/server", icon: Server, id: "server" },
  { label: "Security", href: "/security", icon: Shield, id: "security" },
  { label: "Settings", href: "/settings", icon: Settings, id: "settings" },
];

type PanelShellProps = {
  username: string;
  active: string;
  signedInLabel: string;
  children: React.ReactNode;
};

function NavGroup({
  title,
  items,
  active,
  onNavigate,
}: {
  title: string;
  items: NavItem[];
  active: string;
  onNavigate: () => void;
}) {
  return (
    <>
      <div className="nav-section">{title}</div>
      {items.map(({ label, href, icon: Icon, id }) => (
        <Link
          key={id}
          href={href}
          className={id === active ? "active" : undefined}
          onClick={onNavigate}
        >
          <Icon size={20} strokeWidth={1.8} />
          {label}
        </Link>
      ))}
    </>
  );
}

export function PanelShell({
  username,
  active,
  signedInLabel,
  children,
}: PanelShellProps) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    const onResize = () => {
      if (window.matchMedia("(min-width: 1024px)").matches) setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onResize);
    };
  }, []);

  useEffect(() => {
    document.body.classList.toggle("nav-open", open);
    return () => document.body.classList.remove("nav-open");
  }, [open]);

  return (
    <div className="panel-layout">
      <button
        type="button"
        className="sidebar-backdrop"
        aria-label="Close navigation"
        tabIndex={-1}
        onClick={() => setOpen(false)}
      />
      <aside
        id="panel-sidebar"
        className="sidebar"
        aria-label="Panel navigation"
        aria-hidden={undefined}
      >
        <div>
          <Link href="/dashboard" className="panel-brand">
            <Server size={23} strokeWidth={1.9} />
            <span>CPN Panel</span>
          </Link>
          <div className="server-summary">
            <Network size={20} aria-hidden="true" />
            <div>
              <strong>{username}</strong>
              <span>{signedInLabel}</span>
            </div>
          </div>
          <nav aria-label="Primary navigation">
            <NavGroup
              title="Hosting"
              items={hosting}
              active={active}
              onNavigate={() => setOpen(false)}
            />
            <NavGroup
              title="Account"
              items={account}
              active={active}
              onNavigate={() => setOpen(false)}
            />
            <NavGroup
              title="Administration"
              items={administration}
              active={active}
              onNavigate={() => setOpen(false)}
            />
          </nav>
        </div>
        <div className="sidebar-footer">
          <Link href="/api/logout" className="logout">
            <LogOut size={18} /> Log out
          </Link>
        </div>
      </aside>
      <section className="panel-main">
        <header className="mobile-header">
          <button
            type="button"
            className="icon-btn"
            aria-controls="panel-sidebar"
            aria-expanded={open}
            aria-label={open ? "Close navigation" : "Open navigation"}
            onClick={() => setOpen((value) => !value)}
          >
            {open ? <X size={22} /> : <Menu size={22} />}
          </button>
          <strong>CPN Panel</strong>
          <Link href="/api/logout" className="logout">
            Log out
          </Link>
        </header>
        {children}
      </section>
    </div>
  );
}
