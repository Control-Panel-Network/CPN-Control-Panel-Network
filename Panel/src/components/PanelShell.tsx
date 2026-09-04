"use client";

import Link from "next/link";
import { useEffect, useState, useSyncExternalStore } from "react";
import {
  AppWindow,
  Bell,
  Database,
  Gauge,
  Globe2,
  HardDrive,
  Mail,
  Menu,
  Moon,
  Network,
  Package,
  PanelLeftClose,
  PanelLeftOpen,
  Puzzle,
  Server,
  Settings,
  Shield,
  Sun,
  Users,
  X,
} from "lucide-react";

type NavItem = { label: string; href: string; icon: typeof Gauge; id: string };

const STORAGE_KEY = "cpn-sidebar-collapsed";
const COLOR_MODE_KEY = "cpn-color-mode";
const NOTIFY_KEY = "cpn-panel-notifications";

type PanelNotice = {
  id: string;
  title: string;
  body: string;
  read: boolean;
};

function readColorMode(): "light" | "dark" {
  if (typeof window === "undefined") return "light";
  try {
    const stored = window.localStorage.getItem(COLOR_MODE_KEY);
    return stored === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

function writeColorMode(mode: "light" | "dark") {
  try {
    window.localStorage.setItem(COLOR_MODE_KEY, mode);
  } catch {
    /* ignore */
  }
  document.documentElement.setAttribute("data-color-mode", mode);
  document.body.setAttribute("data-color-mode", mode);
}

function readNotices(): PanelNotice[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(NOTIFY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as PanelNotice[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function writeNotices(items: PanelNotice[]) {
  try {
    window.localStorage.setItem(NOTIFY_KEY, JSON.stringify(items));
  } catch {
    /* ignore */
  }
}
const COLLAPSED_EVENT = "cpn-sidebar-collapsed-change";
const NARROW_MQ = "(max-width: 1023.98px)";

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
  { label: "Packages", href: "/packages", icon: Package, id: "packages" },
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

function readCollapsedPreference(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function subscribeCollapsed(onStoreChange: () => void) {
  window.addEventListener("storage", onStoreChange);
  window.addEventListener(COLLAPSED_EVENT, onStoreChange);
  return () => {
    window.removeEventListener("storage", onStoreChange);
    window.removeEventListener(COLLAPSED_EVENT, onStoreChange);
  };
}

function writeCollapsedPreference(next: boolean) {
  try {
    window.localStorage.setItem(STORAGE_KEY, next ? "1" : "0");
  } catch {
    /* ignore quota / private mode */
  }
  window.dispatchEvent(new Event(COLLAPSED_EVENT));
}

function subscribeNarrow(onStoreChange: () => void) {
  const mq = window.matchMedia(NARROW_MQ);
  mq.addEventListener("change", onStoreChange);
  return () => mq.removeEventListener("change", onStoreChange);
}

function getNarrowSnapshot() {
  return window.matchMedia(NARROW_MQ).matches;
}

export function PanelShell({
  username,
  active,
  signedInLabel,
  children,
}: PanelShellProps) {
  const [open, setOpen] = useState(false);
  const [notifyOpen, setNotifyOpen] = useState(false);
  const [colorMode, setColorMode] = useState<"light" | "dark">("light");
  const [notices, setNotices] = useState<PanelNotice[]>([]);
  const collapsed = useSyncExternalStore(
    subscribeCollapsed,
    readCollapsedPreference,
    () => false,
  );
  const narrow = useSyncExternalStore(
    subscribeNarrow,
    getNarrowSnapshot,
    () => false,
  );

  useEffect(() => {
    setColorMode(readColorMode());
    setNotices(readNotices());
  }, []);

  useEffect(() => {
    document.body.classList.toggle("sidebar-collapsed", collapsed);
    return () => document.body.classList.remove("sidebar-collapsed");
  }, [collapsed]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        setNotifyOpen(false);
      }
    };
    const onResize = () => {
      if (!window.matchMedia(NARROW_MQ).matches && !collapsed) {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onResize);
    };
  }, [collapsed]);

  useEffect(() => {
    document.body.classList.toggle("nav-open", open);
    return () => document.body.classList.remove("nav-open");
  }, [open]);

  const drawerMode = collapsed || narrow;
  const unread = notices.filter((item) => !item.read).length;

  const toggleCollapsed = () => {
    writeCollapsedPreference(!collapsed);
    setOpen(false);
  };

  const toggleColorMode = () => {
    const next = colorMode === "dark" ? "light" : "dark";
    setColorMode(next);
    writeColorMode(next);
  };

  const markAllRead = () => {
    const next = notices.map((item) => ({ ...item, read: true }));
    setNotices(next);
    writeNotices(next);
  };

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
        aria-hidden={drawerMode && !open ? true : undefined}
      >
        <div className="sidebar-header">
          <div className="sidebar-brand-row">
            <Link href="/dashboard" className="panel-brand">
              <Server size={23} strokeWidth={1.9} />
              <span>CPN Panel</span>
            </Link>
            <button
              type="button"
              className="icon-btn sidebar-collapse-btn"
              aria-controls="panel-sidebar"
              aria-pressed={collapsed}
              aria-label={collapsed ? "Show sidebar" : "Hide sidebar"}
              title={collapsed ? "Show sidebar" : "Hide sidebar"}
              onClick={toggleCollapsed}
            >
              {collapsed ? (
                <PanelLeftOpen size={18} strokeWidth={1.9} />
              ) : (
                <PanelLeftClose size={18} strokeWidth={1.9} />
              )}
            </button>
          </div>
          <div className="server-summary">
            <Network size={20} aria-hidden="true" />
            <div>
              <strong>{username}</strong>
              <span>{signedInLabel}</span>
            </div>
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
        <div className="sidebar-footer">
          <div className="sidebar-footer-actions">
            <div className="notify-wrap">
              <button
                type="button"
                className="footer-icon-btn"
                aria-expanded={notifyOpen}
                aria-controls="cpn-notify-panel"
                aria-label="Notifications"
                title="Notifications"
                onClick={() => setNotifyOpen((value) => !value)}
              >
                <Bell size={18} strokeWidth={1.9} />
                {unread > 0 ? (
                  <span className="notify-badge">{unread > 99 ? "99+" : unread}</span>
                ) : null}
              </button>
              {notifyOpen ? (
                <div
                  id="cpn-notify-panel"
                  className="notify-popover"
                  role="dialog"
                  aria-label="Notifications"
                >
                  <header>
                    <span>Notifications</span>
                    <button type="button" onClick={markAllRead}>
                      Mark all read
                    </button>
                  </header>
                  {notices.length === 0 ? (
                    <p className="notify-empty">No notifications yet.</p>
                  ) : (
                    <ul className="notify-list">
                      {notices.map((item) => (
                        <li key={item.id} className={item.read ? undefined : "unread"}>
                          <strong>{item.title}</strong>
                          {item.body ? <span>{item.body}</span> : null}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              ) : null}
            </div>
            <Link
              href="/settings"
              className="footer-icon-btn"
              aria-label="Account settings"
              title="Account settings"
              onClick={() => setOpen(false)}
            >
              <Settings size={18} strokeWidth={1.9} />
            </Link>
            <button
              type="button"
              id="cpn-color-toggle"
              className="theme-toggle footer-icon-btn"
              aria-pressed={colorMode === "dark"}
              aria-label={
                colorMode === "dark" ? "Switch to light mode" : "Switch to dark mode"
              }
              title={
                colorMode === "dark" ? "Switch to light mode" : "Switch to dark mode"
              }
              onClick={toggleColorMode}
            >
              {colorMode === "dark" ? (
                <Sun size={18} strokeWidth={1.9} aria-hidden="true" />
              ) : (
                <Moon size={18} strokeWidth={1.9} aria-hidden="true" />
              )}
            </button>
          </div>
          <Link href="/api/logout" className="logout">
            Log out
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
