/**
 * Build a GitHub "new issue" URL with a sanitized installer failure template.
 * Never includes IP addresses, MACs, tokens, passwords, or usernames.
 */

import type { InstallerStatus } from "./types";

export const GITHUB_ISSUES_NEW =
  "https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/issues/new";

/** Practical limit so links stay usable in browsers and chat clients. */
export const MAX_ISSUE_URL_LENGTH = 6000;
export const MAX_ERROR_CHARS = 800;

const IPV4_RE =
  /\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)\b/g;
const IPV6_RE =
  /\b(?:(?:[0-9a-f]{1,4}:){2,7}[0-9a-f]{0,4}|::(?:[0-9a-f]{1,4}:){0,6}[0-9a-f]{0,4})\b/gi;
const MAC_RE = /\b(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}\b/gi;
const TOKEN_RE =
  /\b(?:Bearer\s+[A-Za-z0-9._\-+=\/]+|cpn_[A-Za-z0-9]+|token[=:]\s*[^\s&]+|password[=:]\s*[^\s&]+|secret[=:]\s*[^\s&]+)\b/gi;

export function sanitizeIssueText(input: string): string {
  return input
    .replace(IPV4_RE, "[redacted-ip]")
    .replace(IPV6_RE, "[redacted-ip]")
    .replace(MAC_RE, "[redacted-mac]")
    .replace(TOKEN_RE, "[redacted-secret]")
    .replace(/\r\n/g, "\n")
    .trim();
}

export function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return `${text.slice(0, Math.max(0, max - 14))}…[truncated]`;
}

export function failedStepFromStatus(status: InstallerStatus): string {
  if (status.phase !== "failed") {
    return status.phase || "unknown";
  }
  if (status.progress >= 90) return "testing";
  if (status.progress >= 80) return "installing";
  if (status.progress > 0) return "downloading";
  return "configuring";
}

function stepLabel(step: string): string {
  switch (step) {
    case "configuring":
      return "Configuring";
    case "downloading":
      return "Downloading";
    case "installing":
      return "Installing";
    case "testing":
      return "Testing";
    default:
      return step;
  }
}

function line(label: string, value: string | null | undefined): string | null {
  const trimmed = (value ?? "").trim();
  if (!trimmed) return null;
  return `- ${label}: ${sanitizeIssueText(trimmed)}`;
}

/** Safe environment lines only (no addresses, hostnames, or public URLs). */
export function buildSafeSystemLines(status: InstallerStatus): string[] {
  const env = status.environment;
  const lines: Array<string | null> = [
    line("CPN version", status.version),
    line("OS", env?.os_pretty_name),
    line("Architecture", env?.arch),
    line("Kernel", env?.kernel),
    line("Install mode", "web"),
    line("Failed step", stepLabel(failedStepFromStatus(status))),
    line("Phase", status.phase),
    line("Progress", String(status.progress ?? 0)),
    line("Selected server", status.selected_server),
    line("Selected mail", status.selected_mail),
    line(
      "Virtualization",
      env?.virtualization
        ? `${env.virtualization} (vps=${env.is_vps ? "yes" : "no"}, container=${env.is_container ? "yes" : "no"})`
        : env
          ? `vps=${env.is_vps ? "yes" : "no"}, container=${env.is_container ? "yes" : "no"}`
          : null,
    ),
    line("Firewall", env?.firewall),
    line(
      "Listen port",
      env?.port != null
        ? String(env.port)
        : status.listen_port != null
          ? String(status.listen_port)
          : null,
    ),
    line("UI language", status.language),
    line("Timestamp (UTC)", new Date().toISOString().replace(/\.\d{3}Z$/, "Z")),
  ];
  return lines.filter((entry): entry is string => Boolean(entry));
}

export function buildIssueTitle(status: InstallerStatus): string {
  const step = stepLabel(failedStepFromStatus(status));
  const err = sanitizeIssueText(
    status.error || status.message || "unknown error",
  );
  const short = truncate(err.replace(/\s+/g, " "), 72);
  return sanitizeIssueText(`[Bug]: Installer failed at ${step}: ${short}`);
}

export function buildIssueBody(status: InstallerStatus): string {
  const step = stepLabel(failedStepFromStatus(status));
  const errorRaw = status.error || status.message || "(no error text)";
  const errorText = truncate(sanitizeIssueText(errorRaw), MAX_ERROR_CHARS);
  const system = buildSafeSystemLines(status).join("\n");

  return [
    "### Summary",
    `Installer UI reported a failure during **${step}**.`,
    "",
    "### Steps to reproduce",
    "1. Start the CPN installer (web UI).",
    "2. Proceed until the failure appears.",
    "3. (Add any extra steps here.)",
    "",
    "### Expected behavior",
    "Installation continues or fails with a clear recoverable error.",
    "",
    "### Actual behavior",
    "```",
    errorText,
    "```",
    "",
    "### Component",
    "Installer UI",
    "",
    "### OS / environment",
    system,
    "",
    "- Host IP addresses, blurred IP widgets, MAC addresses, usernames, passwords, and install tokens are intentionally omitted.",
    "",
    "### Logs and screenshots",
    "Attach `/var/lib/cpn/installation.log` when available (redact secrets).",
    "",
    "### Checklist",
    "- [ ] I searched existing issues and did not find a duplicate.",
    "- [x] I redacted installer tokens, passwords, and other secrets from this report.",
  ].join("\n");
}

export function buildGitHubIssueUrl(status: InstallerStatus): string {
  let title = buildIssueTitle(status);
  let body = buildIssueBody(status);

  const encode = (t: string, b: string) => {
    const params = new URLSearchParams();
    params.set("title", t);
    params.set("body", b);
    return `${GITHUB_ISSUES_NEW}?${params.toString()}`;
  };

  let url = encode(title, body);
  if (url.length <= MAX_ISSUE_URL_LENGTH) {
    return url;
  }

  // Shrink body first, then title, until under the limit.
  let errorBudget = Math.min(MAX_ERROR_CHARS, 400);
  while (url.length > MAX_ISSUE_URL_LENGTH && errorBudget > 80) {
    errorBudget = Math.floor(errorBudget * 0.7);
    const slimStatus: InstallerStatus = {
      ...status,
      error: truncate(sanitizeIssueText(status.error || ""), errorBudget),
      message: "",
    };
    title = buildIssueTitle(slimStatus);
    body = buildIssueBody(slimStatus);
    url = encode(title, body);
  }

  if (url.length > MAX_ISSUE_URL_LENGTH) {
    body = truncate(body, 1500);
    url = encode(title, body);
  }

  if (url.length > MAX_ISSUE_URL_LENGTH) {
    // Last resort: title only.
    const params = new URLSearchParams();
    params.set("title", truncate(title, 120));
    return `${GITHUB_ISSUES_NEW}?${params.toString()}`;
  }

  return url;
}
