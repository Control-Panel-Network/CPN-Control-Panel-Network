"use client";

import { Ban, Bell, Check, Info, TriangleAlert, X } from "lucide-react";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { createPortal } from "react-dom";

export type NotifySeverity = "error" | "warning" | "success" | "info";

export type PanelNotice = {
  id: string;
  title: string;
  body: string;
  read: boolean;
  category?: string;
  severity?: string;
  level?: string;
};

type NotifyPos = {
  bottom: number;
  left: number;
  width: number;
};

const POPOVER_WIDTH = 340;
const POPOVER_GAP = 8;
const VIEWPORT_PAD = 12;

type NotificationsPopoverProps = {
  notices: PanelNotice[];
  onMarkAllRead: () => void;
  onDismiss?: (id: string) => void;
};

function computeNotifyPos(btn: HTMLElement): NotifyPos {
  const rect = btn.getBoundingClientRect();
  const width = Math.min(POPOVER_WIDTH, window.innerWidth - VIEWPORT_PAD * 2);
  let left = rect.left;
  if (left + width > window.innerWidth - VIEWPORT_PAD) {
    left = Math.max(VIEWPORT_PAD, window.innerWidth - width - VIEWPORT_PAD);
  }
  if (left < VIEWPORT_PAD) {
    left = VIEWPORT_PAD;
  }
  const bottom = Math.max(
    VIEWPORT_PAD,
    window.innerHeight - rect.top + POPOVER_GAP,
  );
  return { bottom, left, width };
}

function subscribeMounted() {
  return () => {};
}

function normalizeSeverity(raw: string | undefined): NotifySeverity | "" {
  const v = String(raw || "")
    .toLowerCase()
    .trim();
  if (
    v === "error" ||
    v === "err" ||
    v === "danger" ||
    v === "critical" ||
    v === "fail" ||
    v === "failed"
  ) {
    return "error";
  }
  if (v === "warning" || v === "warn" || v === "caution") {
    return "warning";
  }
  if (v === "success" || v === "ok" || v === "done") {
    return "success";
  }
  if (
    v === "info" ||
    v === "information" ||
    v === "neutral" ||
    v === "notice"
  ) {
    return "info";
  }
  return "";
}

function inferSeverity(item: PanelNotice): NotifySeverity {
  const blob = `${item.category || ""} ${item.title} ${item.body}`.toLowerCase();
  if (/fail|error|denied|critical|unable|could not/.test(blob)) {
    return "error";
  }
  if (
    /expir|warn|attention|soon|degrad/.test(blob) &&
    !/renew|success|installed|completed|created|ready/.test(blob)
  ) {
    return "warning";
  }
  if (/renew|success|installed|completed|created|ready/.test(blob)) {
    return "success";
  }
  return "info";
}

export function resolveNoticeSeverity(item: PanelNotice): NotifySeverity {
  return (
    normalizeSeverity(item.severity) ||
    normalizeSeverity(item.level) ||
    inferSeverity(item)
  );
}

function SeverityIcon({ severity }: { severity: NotifySeverity }) {
  if (severity === "error") {
    return <Ban size={14} strokeWidth={2.4} aria-hidden />;
  }
  if (severity === "warning") {
    return <TriangleAlert size={14} strokeWidth={2.2} aria-hidden />;
  }
  if (severity === "success") {
    return <Check size={14} strokeWidth={2.6} aria-hidden />;
  }
  return <Info size={14} strokeWidth={2.2} aria-hidden />;
}

export function NotificationsPopover({
  notices,
  onMarkAllRead,
  onDismiss,
}: NotificationsPopoverProps) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<NotifyPos | null>(null);
  const mounted = useSyncExternalStore(
    subscribeMounted,
    () => true,
    () => false,
  );
  const btnRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const unread = notices.filter((item) => !item.read).length;

  const close = useCallback(() => {
    setOpen(false);
    setPos(null);
  }, []);

  const openPanel = useCallback(() => {
    const btn = btnRef.current;
    if (!btn) return;
    setPos(computeNotifyPos(btn));
    setOpen(true);
  }, []);

  useEffect(() => {
    if (!open) return;
    const onReposition = () => {
      const btn = btnRef.current;
      if (!btn) return;
      setPos(computeNotifyPos(btn));
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        close();
      }
    };
    const onPointer = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (btnRef.current?.contains(target)) return;
      if (panelRef.current?.contains(target)) return;
      close();
    };
    window.addEventListener("resize", onReposition);
    window.addEventListener("scroll", onReposition, true);
    window.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onPointer);
    return () => {
      window.removeEventListener("resize", onReposition);
      window.removeEventListener("scroll", onReposition, true);
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onPointer);
    };
  }, [open, close]);

  const panel =
    open && mounted && pos
      ? createPortal(
          <div
            ref={panelRef}
            id="cpn-notify-panel"
            className="notify-popover"
            role="dialog"
            aria-label="Notifications"
            style={{
              bottom: pos.bottom,
              left: pos.left,
              width: pos.width,
            }}
          >
            <header>
              <span>Notifications</span>
              <button type="button" onClick={onMarkAllRead}>
                Mark all as read
              </button>
            </header>
            {notices.length === 0 ? (
              <p className="notify-empty">No notifications yet.</p>
            ) : (
              <ul className="notify-list">
                {notices.map((item) => {
                  const severity = resolveNoticeSeverity(item);
                  return (
                    <li key={item.id}>
                      <div
                        className={`notify-toast notify-toast--${severity}${
                          item.read ? " is-read" : ""
                        }`}
                        data-severity={severity}
                      >
                        <span className="notify-toast-icon" aria-hidden>
                          <SeverityIcon severity={severity} />
                        </span>
                        <div className="notify-toast-text">
                          <strong>{item.title}</strong>
                          {item.body ? <span>{item.body}</span> : null}
                        </div>
                        <button
                          type="button"
                          className="notify-toast-dismiss"
                          aria-label="Dismiss"
                          title="Dismiss"
                          onClick={() => onDismiss?.(item.id)}
                        >
                          <X size={12} strokeWidth={2.4} aria-hidden />
                        </button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>,
          document.body,
        )
      : null;

  return (
    <div className="notify-wrap">
      <button
        ref={btnRef}
        type="button"
        className="footer-icon-btn"
        aria-expanded={open}
        aria-controls="cpn-notify-panel"
        aria-label="Notifications"
        title="Notifications"
        onClick={() => {
          if (open) {
            close();
          } else {
            openPanel();
          }
        }}
      >
        <Bell size={18} strokeWidth={1.9} />
        {unread > 0 ? (
          <span className="notify-badge">{unread > 99 ? "99+" : unread}</span>
        ) : null}
      </button>
      {panel}
    </div>
  );
}
