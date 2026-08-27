"use client";

import { useState } from "react";
import { ExternalLink, Mail, Plus, Trash2 } from "lucide-react";

export type Mailbox = { address: string; created_at: number };

export default function EmailManager({ initial, domain }: { initial: Mailbox[]; domain: string }) {
  const [mailboxes, setMailboxes] = useState(initial);
  const [localPart, setLocalPart] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  async function createMailbox(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setMessage("");
    const response = await fetch("/api/mailboxes", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ local_part: localPart, password }),
    });
    const payload = await response.json().catch(() => ({}));
    if (response.ok) {
      setMailboxes((current) => [...current, payload]);
      setLocalPart("");
      setPassword("");
      setMessage("Buzón creado y comprobado con Dovecot.");
    } else setMessage(payload.error ?? "No se pudo crear el buzón.");
    setBusy(false);
  }

  async function removeMailbox(address: string) {
    if (!window.confirm(`Se eliminarán el buzón ${address} y sus mensajes. ¿Continuar?`)) return;
    setBusy(true);
    const response = await fetch("/api/mailboxes", {
      method: "DELETE",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ address }),
    });
    if (response.ok) {
      setMailboxes((current) => current.filter((mailbox) => mailbox.address !== address));
      setMessage("Buzón eliminado.");
    } else {
      const payload = await response.json().catch(() => ({}));
      setMessage(payload.error ?? "No se pudo eliminar el buzón.");
    }
    setBusy(false);
  }

  async function openMailbox(address: string) {
    setBusy(true);
    const response = await fetch("/api/mailboxes/open", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ address }),
    });
    const payload = await response.json().catch(() => ({}));
    setBusy(false);
    if (!response.ok) return setMessage(payload.error ?? "No se pudo abrir el webmail.");
    if (!payload.automatic) setMessage("Este cliente no ofrece SSO seguro; se abrirá su inicio de sesión.");
    window.open(payload.url, "_blank", "noopener,noreferrer");
  }

  return (
    <>
      <form className="mail-create" onSubmit={createMailbox}>
        <label>
          Dirección
          <span className="mail-address-input">
            <input value={localPart} onChange={(event) => setLocalPart(event.target.value)} required pattern="[A-Za-z0-9._-]+" />
            <span>@{domain}</span>
          </span>
        </label>
        <label>
          Contraseña inicial
          <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} minLength={12} required autoComplete="new-password" />
        </label>
        <button disabled={busy}><Plus size={18} /> Crear buzón</button>
      </form>

      {message && <p className="mail-message" role="status">{message}</p>}
      <div className="mail-list">
        {mailboxes.length === 0 && <p className="empty-mail"><Mail size={24} /> Todavía no hay buzones.</p>}
        {mailboxes.map((mailbox) => (
          <article key={mailbox.address}>
            <div><Mail size={20} /><strong>{mailbox.address}</strong></div>
            <div className="mail-actions">
              <button onClick={() => openMailbox(mailbox.address)} disabled={busy}><ExternalLink size={17} /> Abrir correo</button>
              <button className="danger" onClick={() => removeMailbox(mailbox.address)} disabled={busy} aria-label={`Eliminar ${mailbox.address}`}><Trash2 size={17} /></button>
            </div>
          </article>
        ))}
      </div>
    </>
  );
}
