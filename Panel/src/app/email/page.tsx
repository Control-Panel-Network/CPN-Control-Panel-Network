import Link from "next/link";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { ArrowLeft, Mail } from "lucide-react";
import { PANEL_COOKIE, validSession } from "@/lib/panel-auth";
import { mailboxes as listMailboxes, systemInfo } from "@/lib/system-manager";
import EmailManager, { type Mailbox } from "./email-manager";

type SystemInfo = { domain: string; webmail: string };

export default async function EmailPage() {
  const store = await cookies();
  if (!validSession(store.get(PANEL_COOKIE)?.value)) redirect("/");
  const [system, mailboxItems] = await Promise.all([
    systemInfo() as Promise<SystemInfo>,
    listMailboxes() as Promise<Mailbox[]>,
  ]);
  return (
    <main className="mail-page">
      <header>
        <Link href="/dashboard"><ArrowLeft size={19} /> Dashboard</Link>
        <span><Mail size={19} /> Correo</span>
      </header>
      <section className="mail-shell">
        <p className="eyebrow">ADMINISTRACIÓN DE CORREO</p>
        <h1>Buzones</h1>
        <p>Crea, elimina y abre las cuentas de <strong>{system.domain}</strong>. Cliente instalado: {system.webmail}.</p>
        <EmailManager initial={mailboxItems} domain={system.domain} />
      </section>
    </main>
  );
}
