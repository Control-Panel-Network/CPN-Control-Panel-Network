"use client";

import { Bell } from "lucide-react";
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
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

function subscribeMounted() {
  return () => {};
}

export function NotificationsPopover({
  notices,
  onMarkAllRead,
}: NotificationsPopoverProps) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<NotifyPos | null>(null);
  const mounted = useSyncExternalStore(subscribeMounted, () => true, () => false);
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
