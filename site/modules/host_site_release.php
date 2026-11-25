<?php
/**
 * Resolve latest CPN release from GitHub (public API) with file cache and fallback.
 * PHP 7.4+. No secrets required for the landing page.
 */

declare(strict_types=1);

function cpn_host_site_root(): string
{
    return dirname(__DIR__);
}

/**
 * @return array<string, mixed>
 */
function cpn_host_site_release_config(): array
{
    $defaults = [
        'github_owner' => 'Control-Panel-Network',
        'github_repo' => 'CPN-Control-Panel-Network',
        'fallback_version' => '1.4.10',
        'cache_ttl_seconds' => 600,
        'cache_file' => cpn_host_site_root() . '/data/cache/github-latest-release.json',
        'log_file' => cpn_host_site_root() . '/data/logs/host-site-release.log',
        'github_user_agent' => 'CPN-Host-Site/1.0 (+https://cpn.newstargeted.com)',
    ];

    $configPath = cpn_host_site_root() . '/config.host-site.php';
    if (is_file($configPath)) {
        $custom = require $configPath;
        if (is_array($custom)) {
            return array_merge($defaults, $custom);
        }
    }

    return $defaults;
}

function cpn_host_site_log(string $message): void
{
    $cfg = cpn_host_site_release_config();
    $line = gmdate('Y-m-d\TH:i:s\Z') . ' ' . $message . "\n";
    $logFile = $cfg['log_file'];
    $dir = dirname($logFile);
    if (!is_dir($dir)) {
        @mkdir($dir, 0750, true);
    }
    @file_put_contents($logFile, $line, FILE_APPEND | LOCK_EX);
}

function cpn_host_site_normalize_version(string $tag): string
{
    $tag = trim($tag);
    if ($tag === '') {
        return '';
    }
    if ($tag[0] === 'v' || $tag[0] === 'V') {
        return substr($tag, 1);
    }
    return $tag;
}

/**
 * @return array<string, mixed>
 */
function cpn_host_site_fallback_release(string $source = 'fallback'): array
{
    $cfg = cpn_host_site_release_config();
    $version = (string) $cfg['fallback_version'];

    return [
        'version' => $version,
        'tag' => 'v' . $version,
        'source' => $source,
        'fetched_at' => time(),
        'cache_ttl_seconds' => (int) $cfg['cache_ttl_seconds'],
    ];
}

/**
 * @return array<string, mixed>|null
 */
function cpn_host_site_read_cache(array $cfg): ?array
{
    $path = (string) $cfg['cache_file'];
    if (!is_readable($path)) {
        return null;
    }
    $raw = file_get_contents($path);
    if ($raw === false || $raw === '') {
        return null;
    }
    $data = json_decode($raw, true);
    if (!is_array($data) || empty($data['version'])) {
        return null;
    }
    return $data;
}

/**
 * @param array<string, mixed> $data
 */
function cpn_host_site_write_cache(array $cfg, array $data): void
{
    $path = (string) $cfg['cache_file'];
    $dir = dirname($path);
    if (!is_dir($dir)) {
        if (!mkdir($dir, 0750, true) && !is_dir($dir)) {
            cpn_host_site_log('cache mkdir failed: ' . $dir);
            return;
        }
    }
    $payload = json_encode($data, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    if ($payload === false) {
        return;
    }
    if (@file_put_contents($path, $payload, LOCK_EX) === false) {
        cpn_host_site_log('cache write failed: ' . $path);
    }
    @chmod($path, 0640);
}

/**
 * @return array<string, mixed>|null
 */
function cpn_host_site_fetch_github(array $cfg): ?array
{
    $owner = rawurlencode((string) $cfg['github_owner']);
    $repo = rawurlencode((string) $cfg['github_repo']);
    $url = 'https://api.github.com/repos/' . $owner . '/' . $repo . '/releases/latest';

    $context = stream_context_create([
        'http' => [
            'method' => 'GET',
            'timeout' => 12,
            'header' => implode("\r\n", [
                'Accept: application/vnd.github+json',
                'User-Agent: ' . (string) $cfg['github_user_agent'],
                'X-GitHub-Api-Version: 2022-11-28',
            ]),
        ],
        'ssl' => [
            'verify_peer' => true,
            'verify_peer_name' => true,
        ],
    ]);

    $raw = @file_get_contents($url, false, $context);
    if ($raw === false) {
        cpn_host_site_log('GitHub fetch failed (network)');
        return null;
    }

    $status = 0;
    if (isset($http_response_header[0]) && preg_match('/\s(\d{3})\s/', $http_response_header[0], $m)) {
        $status = (int) $m[1];
    }
    if ($status === 403 || $status === 429) {
        cpn_host_site_log('GitHub rate limited HTTP ' . $status);
        return null;
    }
    if ($status < 200 || $status >= 300) {
        cpn_host_site_log('GitHub HTTP ' . $status);
        return null;
    }

    $json = json_decode($raw, true);
    if (!is_array($json)) {
        cpn_host_site_log('GitHub JSON decode failed');
        return null;
    }
    if (!empty($json['draft']) || !empty($json['prerelease'])) {
        cpn_host_site_log('GitHub latest is draft/prerelease; skipped');
        return null;
    }
    $tag = isset($json['tag_name']) ? (string) $json['tag_name'] : '';
    $version = cpn_host_site_normalize_version($tag);
    if ($version === '') {
        cpn_host_site_log('GitHub missing tag_name');
        return null;
    }

    return [
        'version' => $version,
        'tag' => $tag !== '' ? $tag : ('v' . $version),
        'source' => 'github',
        'fetched_at' => time(),
        'cache_ttl_seconds' => (int) $cfg['cache_ttl_seconds'],
        'html_url' => isset($json['html_url']) ? (string) $json['html_url'] : null,
        'published_at' => isset($json['published_at']) ? (string) $json['published_at'] : null,
    ];
}

/**
 * @return array<string, mixed>
 */
function cpn_host_site_latest_release(): array
{
    $cfg = cpn_host_site_release_config();
    $ttl = (int) $cfg['cache_ttl_seconds'];
    if ($ttl < 60) {
        $ttl = 60;
    }

    $cached = cpn_host_site_read_cache($cfg);
    $now = time();
    if (is_array($cached) && !empty($cached['fetched_at'])) {
        $age = $now - (int) $cached['fetched_at'];
        if ($age >= 0 && $age < $ttl) {
            $cached['source'] = 'cache';
            return $cached;
        }
    }

    $fresh = cpn_host_site_fetch_github($cfg);
    if (is_array($fresh)) {
        cpn_host_site_write_cache($cfg, $fresh);
        return $fresh;
    }

    if (is_array($cached) && !empty($cached['version'])) {
        $cached['source'] = 'stale-cache';
        return $cached;
    }

    return cpn_host_site_fallback_release('fallback');
}