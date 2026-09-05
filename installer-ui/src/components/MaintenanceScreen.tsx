import { useMemo, useState } from 'react';
import { useI18n } from '../i18n';
import type { MaintenanceAction, MaintenanceInfo } from '../types';

export function MaintenanceScreen({
  info,
  busy,
  error,
  onAction,
}: {
  info: MaintenanceInfo;
  busy: boolean;
  error?: string | null;
  onAction: (action: MaintenanceAction, version?: string, confirmDowngrade?: boolean) => void;
}) {
  const { t } = useI18n();
  const [selectedVersion, setSelectedVersion] = useState(
    info.latest_version || info.installed_version,
  );
  const [confirmDowngrade, setConfirmDowngrade] = useState(false);
  const releases = info.releases ?? [];

  const versionNote = useMemo(() => {
    if (info.update_available && info.latest_version) {
      return t.maintenanceUpdateAvailable
        .replace('{installed}', info.installed_version)
        .replace('{latest}', info.latest_version);
    }
    return t.maintenanceUpToDate.replace('{version}', info.installed_version);
  }, [info, t]);

  const plan = info.plan;
  const isOlder =
    selectedVersion !== info.installed_version
    && releases.some((release) => release.version === selectedVersion)
    && selectedVersion.localeCompare(info.installed_version, undefined, { numeric: true }) < 0;

  return (
    <section className="min-h-screen px-6 py-10 grid place-items-center">
      <div className="w-full max-w-2xl">
        <p className="eyebrow">{t.maintenanceEyebrow}</p>
        <h1 className="text-4xl sm:text-5xl font-semibold tracking-[-0.04em] mt-2">
          {t.maintenanceTitle}
        </h1>
        <p className="text-[#667085] text-lg mt-4">{t.maintenanceIntro}</p>
        <p className="text-[#344054] text-base mt-3 font-semibold">{versionNote}</p>
        {info.check_error ? (
          <p className="error-box mt-4">{info.check_error}</p>
        ) : null}

        <div className="panel p-6 mt-8 text-left">
          <label className="field">
            <span>{t.maintenanceChooseVersion}</span>
            <select
              className="field-input"
              value={selectedVersion}
              disabled={busy}
              onChange={(event) => setSelectedVersion(event.target.value)}
            >
              {releases.length === 0 ? (
                <option value={info.installed_version}>{info.installed_version}</option>
              ) : (
                releases.map((release) => (
                  <option key={release.tag_name} value={release.version}>
                    {release.tag_name}
                    {release.prerelease ? ' (pre)' : ''}
                    {release.published_at ? ` · ${release.published_at}` : ''}
                  </option>
                ))
              )}
            </select>
          </label>

          {isOlder ? (
            <label className="mt-4 flex items-start gap-3 text-sm text-[#344054]">
              <input
                type="checkbox"
                checked={confirmDowngrade}
                disabled={busy}
                onChange={(event) => setConfirmDowngrade(event.target.checked)}
              />
              <span>{t.maintenanceConfirmDowngrade}</span>
            </label>
          ) : null}

          {plan ? (
            <div className="mt-6 grid gap-4 sm:grid-cols-2 text-sm">
              <div>
                <p className="font-semibold text-[#147a62]">{t.maintenanceOverwrite}</p>
                <ul className="mt-2 space-y-1 text-[#475467]">
                  {plan.overwrite_paths.slice(0, 8).map((path) => (
                    <li key={path}><code>{path}</code></li>
                  ))}
                </ul>
              </div>
              <div>
                <p className="font-semibold text-[#147a62]">{t.maintenancePreserve}</p>
                <ul className="mt-2 space-y-1 text-[#475467]">
                  {plan.preserve_paths.slice(0, 8).map((path) => (
                    <li key={path}><code>{path}</code></li>
                  ))}
                </ul>
              </div>
            </div>
          ) : null}

          <div className="mt-8 grid gap-3">
            <button
              type="button"
              className="primary-button w-full"
              disabled={busy || !info.update_available}
              onClick={() => onAction('upgrade', info.latest_version || selectedVersion)}
            >
              {t.maintenanceUpgradeLatest}
            </button>
            <button
              type="button"
              className="secondary-button w-full"
              disabled={busy || (isOlder && !confirmDowngrade)}
              onClick={() =>
                onAction(
                  isOlder ? 'downgrade' : 'upgrade',
                  selectedVersion,
                  isOlder ? confirmDowngrade : false,
                )
              }
            >
              {t.maintenanceChooseVersionAction}
            </button>
            <button
              type="button"
              className="secondary-button w-full"
              disabled={busy}
              onClick={() => onAction('repair', selectedVersion, true)}
            >
              {t.maintenanceRepair}
            </button>
            <button
              type="button"
              className="secondary-button w-full"
              disabled={busy}
              onClick={() => onAction('config_only')}
            >
              {t.maintenanceConfigOnly}
            </button>
          </div>
          {error ? <p className="error-box mt-6">{error}</p> : null}
          {busy ? <p className="text-[#667085] text-sm mt-4">{t.maintenanceBusy}</p> : null}
        </div>
      </div>
    </section>
  );
}
