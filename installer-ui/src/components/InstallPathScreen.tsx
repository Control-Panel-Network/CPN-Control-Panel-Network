import { useState } from "react";
import type { ForeignPanelPublic, InstallPathMode } from "../types";
import { useI18n } from "../i18n";

interface Props {
  panels: ForeignPanelPublic[];
  initialTargetId?: string | null;
  onConfirm: (mode: InstallPathMode, targetPanelId?: string) => Promise<void>;
}

export function InstallPathScreen({
  panels,
  initialTargetId,
  onConfirm,
}: Props) {
  const { t } = useI18n();
  const defaultTarget =
    initialTargetId ||
    panels.find((row) => row.id === "classic")?.id ||
    panels[0]?.id ||
    "classic";
  const [targetId, setTargetId] = useState(defaultTarget);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const choose = async (mode: InstallPathMode) => {
    setBusy(true);
    setError(null);
    try {
      await onConfirm(
        mode,
        mode === "migrate" ? targetId : undefined,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : t.unknownError);
      setBusy(false);
    }
  };

  return (
    <section className="min-h-screen grid place-items-center px-6 py-16">
      <div className="max-w-2xl w-full bg-white border border-[#e4e7ec] rounded-2xl p-8 shadow-sm">
        <p className="eyebrow">{t.installPathEyebrow}</p>
        <h1 className="text-3xl font-semibold tracking-tight mt-2">
          {t.installPathTitle}
        </h1>
        <p className="text-[#667085] mt-3">{t.installPathIntro}</p>
        <ul className="mt-6 space-y-2 text-sm">
          {panels.map((panel) => (
            <li
              key={panel.id}
              className="rounded-xl border border-[#e4e7ec] px-4 py-3 bg-[#f9fafb]"
            >
              <strong>{panel.name}</strong>{" "}
              <span className="text-[#667085]">
                (id={panel.id}, tier={panel.tier}
                {panel.website_count > 0
                  ? `, sites: ${panel.website_count}`
                  : ""}
                )
              </span>
            </li>
          ))}
        </ul>
        {panels.length > 1 && (
          <label className="block mt-6 text-sm font-medium">
            {t.installPathTargetLabel}
            <select
              className="mt-2 w-full rounded-lg border border-[#d0d5dd] px-3 py-2"
              value={targetId}
              disabled={busy}
              onChange={(event) => setTargetId(event.target.value)}
            >
              {panels.map((panel) => (
                <option key={panel.id} value={panel.id}>
                  {panel.name} ({panel.id})
                </option>
              ))}
            </select>
          </label>
        )}
        {error && <p className="error-box mt-6">{error}</p>}
        <div className="mt-8 flex flex-col sm:flex-row gap-3">
          <button
            type="button"
            className="btn-secondary flex-1"
            disabled={busy}
            onClick={() => void choose("fresh")}
          >
            {t.installPathFresh}
          </button>
          <button
            type="button"
            className="btn-primary flex-1"
            disabled={busy}
            onClick={() => void choose("migrate")}
          >
            {t.installPathMigrate}
          </button>
        </div>
        <p className="text-xs text-[#667085] mt-4">{t.installPathMigrateHint}</p>
      </div>
    </section>
  );
}