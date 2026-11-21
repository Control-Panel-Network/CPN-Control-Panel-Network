<?php
declare(strict_types=1);

require dirname(__DIR__) . '/modules/host_site_release.php';

header('Content-Type: application/json; charset=utf-8');
header('X-Content-Type-Options: nosniff');
header('Cache-Control: public, max-age=120, stale-while-revalidate=300');

try {
    $release = cpn_host_site_latest_release();
    echo json_encode($release, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
} catch (Throwable $e) {
    cpn_host_site_log('api/release-version: ' . $e->getMessage());
    http_response_code(503);
    echo json_encode([
        'error' => 'release_unavailable',
        'release' => cpn_host_site_fallback_release('fallback'),
    ], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
}