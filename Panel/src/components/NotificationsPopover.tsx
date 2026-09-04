"use client";

import { Bell } from "lucide-react";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

export type PanelNotice = {
  id: string;
  title: string;
  body: string;
  read: boolean;
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
  const bottom = Math.max(VIEWPORT_PAD, window.innerHeight - rect.top + POPOVER_GAP);
  return { bottom, left, width };
}

export function NotificationsPopover({
  notices,
  onMarkAllRead,
}: NotificationsPopoverProps) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<NotifyPos | null>(null);
  const [mounted, setMounted] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const unread = notices.filter((item) => !item.read).length;

  useEffect(() => {
    setMounted(true);
  }, []);

  const reposition = useCallback(() => {
    const btn = btnRef.current;
    if (!btn) return;
    setPos(computeNotifyPos(btn));
  }, []);

  useLayoutEffect(() => {
    if (!open) {
      setPos(null);
      return;
    }
    reposition();
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [open, reposition]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    const onPointer = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (btnRef.current?.contains(target)) return;
      if (panelRef.current?.contains(target)) return;
      setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onPointer);
    };
  }, [open]);

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
                {notices.map((item) => (
                  <li key={item.id} className={item.read ? undefined : "unread"}>
                    <strong>{item.title}</strong>
                    {item.body ? <span>{item.body}</span> : null}
                  </li>
                ))}
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
        onClick={() => setOpen((value) => !value)}
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
