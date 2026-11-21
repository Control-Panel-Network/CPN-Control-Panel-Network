<?php
/**
 * Host site settings for cpn.newstargeted.com (copy to config.host-site.php on the server).
 * Do not commit config.host-site.php; it may override cache TTL and fallback version only.
 */

return [
    'github_owner' => 'Control-Panel-Network',
    'github_repo' => 'CPN-Control-Panel-Network',
    // Shown when GitHub and cache are both unavailable (keep aligned with Cargo.toml on stable).
    'fallback_version' => '1.4.10',
    // Public unauthenticated GitHub API: keep TTL >= 5 minutes.
    'cache_ttl_seconds' => 600,
];