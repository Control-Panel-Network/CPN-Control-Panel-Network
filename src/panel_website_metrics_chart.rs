//! SVG charts with Y scale (0-100%) and X time axis for host metric samples.

use crate::panel_website_metrics_ring::MetricSample;
use crate::panel_website_resources::unix_now;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt_clock(ts: u64) -> String {
    // Prefer host local clock labels when `date` works; fall back to elapsed minutes.
    #[cfg(not(windows))]
    {
        if let Ok(out) = std::process::Command::new("date")
            .args(["-d", &format!("@{ts}"), "+%H:%M"])
            .env("LC_ALL", "C")
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return s;
                }
            }
        }
    }
    let now = unix_now();
    let ago = now.saturating_sub(ts) / 60;
    if ago == 0 {
        "now".into()
    } else {
        format!("-{ago}m")
    }
}

/// Build an SVG with 0-100% Y labels, time ticks on X, and a real sample polyline.
pub fn metrics_chart_svg(samples: &[MetricSample], kind: &str, stroke: &str) -> String {
    let values: Vec<f32> = samples
        .iter()
        .map(|s| {
            if kind == "mem" {
                s.mem
            } else {
                s.cpu
            }
            .clamp(0.0, 100.0)
        })
        .collect();
    let width = 360.0f32;
    let height = 120.0f32;
    let left = 36.0f32;
    let right = 12.0f32;
    let top = 10.0f32;
    let bottom = 24.0f32;
    let plot_w = width - left - right;
    let plot_h = height - top - bottom;

    let mut paths = String::new();
    // Grid + Y labels
    for pct in [0.0f32, 25.0, 50.0, 75.0, 100.0] {
        let y = top + plot_h - (pct / 100.0) * plot_h;
        paths.push_str(&format!(
            r#"<line x1="{left}" y1="{y:.1}" x2="{x2:.1}" y2="{y:.1}" stroke="#2a2f3a" stroke-width="1"/>"#,
            x2 = left + plot_w,
        ));
        paths.push_str(&format!(
            r#"<text x="2" y="{ty:.1}" fill="#98a2b3" font-size="9">{pct:.0}</text>"#,
            ty = y + 3.0,
            pct = pct,
        ));
    }

    if values.is_empty() {
        paths.push_str(&format!(
            r#"<text x="{cx:.1}" y="{cy:.1}" fill="#98a2b3" font-size="11" text-anchor="middle">Collecting samples...</text>"#,
            cx = left + plot_w / 2.0,
            cy = top + plot_h / 2.0,
        ));
    } else {
        let n = values.len().max(1) as f32;
        let mut d = String::new();
        for (i, v) in values.iter().enumerate() {
            let x = if values.len() == 1 {
                left + plot_w / 2.0
            } else {
                left + (i as f32 / (n - 1.0)) * plot_w
            };
            let y = top + plot_h - (*v / 100.0) * plot_h;
            if i == 0 {
                d.push_str(&format!("M {x:.1} {y:.1}"));
            } else {
                d.push_str(&format!(" L {x:.1} {y:.1}"));
            }
        }
        paths.push_str(&format!(
            r#"<path d="{d}" fill="none" stroke="{stroke}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>"#
        ));

        // X time labels: first, mid, last
        let idxs: Vec<usize> = if samples.len() == 1 {
            vec![0]
        } else if samples.len() == 2 {
            vec![0, samples.len() - 1]
        } else {
            vec![0, samples.len() / 2, samples.len() - 1]
        };
        for idx in idxs {
            let x = if samples.len() == 1 {
                left + plot_w / 2.0
            } else {
                left + (idx as f32 / (samples.len() - 1) as f32) * plot_w
            };
            let label = html_escape(&fmt_clock(samples[idx].t));
            paths.push_str(&format!(
                r#"<text x="{x:.1}" y="{y:.1}" fill="#98a2b3" font-size="9" text-anchor="middle">{label}</text>"#,
                y = height - 6.0,
            ));
        }
    }

    let aria = if kind == "mem" {
        "Host memory usage over recent samples"
    } else {
        "Host CPU usage over recent samples"
    };
    format!(
        r#"<svg class="manage-metric-svg" viewBox="0 0 {width} {height}" role="img" aria-label="{aria}" preserveAspectRatio="none">{paths}</svg>"#,
        width = width,
        height = height,
        aria = html_escape(aria),
        paths = paths,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chart_has_axes() {
        let svg = metrics_chart_svg(&[], "cpu", "#12b76a");
        assert!(svg.contains("100"));
        assert!(svg.contains("0"));
        assert!(svg.contains("Collecting"));
    }

    #[test]
    fn chart_with_samples_has_path() {
        let samples = vec![
            MetricSample {
                t: 1_000,
                cpu: 10.0,
                mem: 20.0,
            },
            MetricSample {
                t: 1_010,
                cpu: 40.0,
                mem: 30.0,
            },
        ];
        let svg = metrics_chart_svg(&samples, "cpu", "#f79009");
        assert!(svg.contains("<path"));
        assert!(svg.contains("#f79009"));
    }
}
