<?php
declare(strict_types=1);

require __DIR__ . '/modules/host_site_release.php';

try {
    $release = cpn_host_site_latest_release();
} catch (Throwable $e) {
    cpn_host_site_log('index.php: ' . $e->getMessage());
    $release = cpn_host_site_fallback_release('fallback');
}

$version = (string) $release['version'];
$tag = (string) ($release['tag'] ?? ('v' . $version));

$template = __DIR__ . '/index.html';
if (!is_readable($template)) {
    http_response_code(500);
    header('Content-Type: text/plain; charset=utf-8');
    echo 'Landing template missing.';
    exit;
}

$html = (string) file_get_contents($template);
$replacements = [
    '__CPN_VERSION__' => htmlspecialchars($version, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8'),
    '__CPN_TAG__' => htmlspecialchars($tag, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8'),
];
$html = str_replace(array_keys($replacements), array_values($replacements), $html);

header('Content-Type: text/html; charset=utf-8');
header('X-Content-Type-Options: nosniff');
header('Cache-Control: public, max-age=300, stale-while-revalidate=600');
echo $html;