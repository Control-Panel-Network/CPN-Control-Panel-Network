/**
 * Smoke checks for githubIssue URL builder (run: npx tsx src/githubIssue.smoke.ts).
 * Not a dedicated Test/ tree: verifies the real module in place.
 */
import {
  MAX_ISSUE_URL_LENGTH,
  buildGitHubIssueUrl,
  buildIssueBody,
  buildSafeSystemLines,
  sanitizeIssueText,
} from "./githubIssue.ts";
import type { InstallerStatus } from "./types.ts";

function assert(cond: unknown, message: string): void {
  if (!cond) {
    throw new Error(message);
  }
}

const sample: InstallerStatus = {
  phase: "failed",
  progress: 0,
  message: "Please keep this window open.",
  selected_server: "openlitespeed",
  selected_mail: null,
  environment: {
    is_vps: true,
    is_container: false,
    virtualization: "kvm",
    firewall: "firewalld",
    port: 2087,
    addresses: ["203.0.113.10", "2001:db8::1", "10.0.2.15"],
    os_pretty_name: "AlmaLinux 9.6",
    arch: "x86_64",
    kernel: "5.14.0-570.el9.x86_64",
  },
  error:
    "Could not query the installer at http://127.0.0.1:2090/api/status token=abc123 password=secret",
  language: "en",
  listen_port: 2087,
  version: "0.2.6-alpha.39",
  password_policy: {
    min_length: 12,
    require_special: true,
    require_uppercase: true,
    require_number: true,
  },
};

assert(
  sanitizeIssueText("fail at 127.0.0.1 and 10.0.2.15").includes(
    "[redacted-ip]",
  ),
  "IPv4 must be redacted",
);
assert(
  !sanitizeIssueText("fail at 127.0.0.1").includes("127.0.0.1"),
  "literal IPv4 must not remain",
);

const body = buildIssueBody(sample);
assert(body.includes("AlmaLinux 9.6"), "OS must appear");
assert(body.includes("x86_64"), "arch must appear");
assert(body.includes("Configuring"), "failed step must appear");
assert(body.includes("[redacted-ip]"), "error IPs redacted in body");
assert(!body.includes("203.0.113.10"), "addresses must not be listed");
assert(!body.includes("10.0.2.15"), "private IP must not appear");
assert(!body.includes("2001:db8"), "IPv6 must not appear");
assert(!body.includes("password=secret"), "password must be redacted");
assert(!body.includes("token=abc123"), "token must be redacted");
assert(
  buildSafeSystemLines(sample).every(
    (line) => !/\d+\.\d+\.\d+\.\d+/.test(line),
  ),
  "system lines must not contain IPv4",
);

const url = buildGitHubIssueUrl(sample);
assert(url.startsWith("https://github.com/Control-Panel-Network/"), "repo URL");
assert(url.includes("title="), "title query param");
assert(url.includes("body="), "body query param");
assert(url.length <= MAX_ISSUE_URL_LENGTH, "URL under length cap");
assert(
  !url.includes("203.0.113"),
  "encoded URL must not include sample public IP",
);
assert(
  !decodeURIComponent(url).includes("10.0.2.15"),
  "decoded URL no private IP",
);

console.log("githubIssue.smoke.ts: ok");
console.log(`url_length=${url.length}`);
