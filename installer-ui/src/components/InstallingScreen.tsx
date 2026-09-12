import { CheckCircle2, Circle, LoaderCircle, XCircle } from "lucide-react";
import type { InstallerStatus } from "../types";
import { useI18n } from "../i18n";

function StepIcon({
  state,
}: {
  state: "pending" | "active" | "done" | "error";
}) {
  if (state === "active")
    return <LoaderCircle size={24} className="text-[#0071e3] animate-spin" />;
  if (state === "done")
    return <CheckCircle2 size={24} className="text-[#0071e3]" />;
  if (state === "error")
    return <XCircle size={24} className="text-[#c2413b]" />;
  return <Circle size={24} className="text-[#a1a1a6]" />;
}

export function InstallingScreen({ status }: { status: InstallerStatus }) {
  const { t, locale } = useI18n();
  const phase = status.phase;
  const failed = phase === "failed";
  const downloading = phase === "downloading";
  const installing = phase === "installing";
  const testing = phase === "testing";
  const failedStep = failed
    ? status.progress >= 90
      ? "testing"
      : status.progress >= 80
        ? "installing"
        : status.progress > 0
          ? "downloading"
          : "configuring"
    : null;
  const downloadDone =
    installing ||
    testing ||
    phase === "completed" ||
    failedStep === "installing" ||
    failedStep === "testing";
  const installDone =
    testing || phase === "completed" || failedStep === "testing";
  const labelClass = (active: boolean, done = false) =>
    `text-[17px] ${active ? "font-semibold text-[#1a1c1d]" : done ? "font-normal text-[#1a1c1d]" : "font-normal text-[#7a7a7a]"}`;

  return (
    <section className="w-full min-h-screen flex flex-col items-center justify-center p-6 bg-white">
      <div className="w-full max-w-md">
        <h1 className="text-[34px] leading-[1.47] font-semibold text-[#1a1c1d] mb-12 text-left tracking-tighter">
          {t.installingTitle}
        </h1>

        <div className="flex flex-col gap-8">
          <div className="flex items-center gap-[17px]">
            <StepIcon
              state={
                phase === "configuring"
                  ? "active"
                  : failedStep === "configuring"
                    ? "error"
                    : "done"
              }
            />
            <span
              className={labelClass(
                phase === "configuring",
                phase !== "configuring" && failedStep !== "configuring",
              )}
            >
              {locale === "es"
                ? "Configurando"
                : locale === "nb"
                  ? "Konfigurerer"
                  : "Configuring"}
            </span>
          </div>
          <div className="flex items-center gap-[17px]">
            <StepIcon
              state={
                downloading
                  ? "active"
                  : failedStep === "downloading"
                    ? "error"
                    : downloadDone
                      ? "done"
                      : "pending"
              }
            />
            <span className={labelClass(downloading, downloadDone)}>
              {downloading
                ? `${t.phaseDownloading} ${status.progress}%`
                : t.phaseDownloading}
            </span>
          </div>

          <div className="flex items-center gap-[17px]">
            <StepIcon
              state={
                installing
                  ? "active"
                  : installDone
                    ? "done"
                    : failedStep === "installing"
                      ? "error"
                      : "pending"
              }
            />
            <span className={labelClass(installing, installDone)}>
              {installing
                ? `${t.phaseInstalling} ${status.progress}%`
                : t.phaseInstalling}
            </span>
          </div>

          <div className="flex items-center gap-[17px]">
            <StepIcon
              state={
                testing
                  ? "active"
                  : phase === "completed"
                    ? "done"
                    : failedStep === "testing"
                      ? "error"
                      : "pending"
              }
            />
            <span className={labelClass(testing, phase === "completed")}>
              {t.phaseTesting}
            </span>
          </div>
        </div>

        <p
          className="mt-10 text-[14px] text-[#7a7a7a] min-h-5"
          aria-live="polite"
        >
          {status.message || t.installingSubtitle}
        </p>
        {phase === "configuring" && (
          <p className="install-wait-note" aria-live="polite">
            {locale === "es"
              ? "Esto puede tomar un rato. Salida silenciosa del gestor de paquetes es normal."
              : locale === "nb"
                ? "Dette kan ta en stund. Stille pakkeutdata er normalt."
                : "This may take a while. Quiet package-manager output is normal, not a hang."}
          </p>
        )}
        {failed && (
          <div className="mt-3">
            <p className="whitespace-pre-wrap break-words text-[14px] text-[#c2413b]">
              {status.error}
            </p>
            <p className="install-wait-note">
              {locale === "es"
                ? "Adjunta /var/lib/cpn/installation.log al issue; contiene todos los comandos y verificaciones."
                : locale === "nb"
                  ? "Legg ved /var/lib/cpn/installation.log i saken; den inneholder alle kommandoer og verifiseringer."
                  : "Attach /var/lib/cpn/installation.log to the issue; it contains every command and verification."}
            </p>
            <a
              className="issue-link"
              href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/issues"
              target="_blank"
              rel="noreferrer"
            >
              {locale === "es"
                ? "Abrir issue en GitHub"
                : locale === "nb"
                  ? "Åpne issue på GitHub"
                  : "Open GitHub issue"}
            </a>
          </div>
        )}
      </div>
    </section>
  );
}
