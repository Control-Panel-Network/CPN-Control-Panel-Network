//! Live dashboard usage gauges (polling; issue #621).

use crate::installer::AppState;
use crate::panel_dashboard::{gauge_stroke_for_usage, host_gauges_snapshot};
use crate::panel_hub_http::require_panel_user;
use crate::panel_user_prefs::load_user_minimalist_mode;
use actix_web::{HttpRequest, HttpResponse, get, web};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Authenticated JSON snapshot for dashboard resource gauges.
#[get("/api/dashboard/metrics")]
pub async fn dashboard_metrics(http: HttpRequest, state: web::Data<Arc<AppState>>) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return HttpResponse::Unauthorized()
            .content_type("application/json; charset=utf-8")
            .body(r#"{"ok":false,"error":"Sign in required"}"#);
    };
    let minimalist = load_user_minimalist_mode(&user);
    let snap = web::block(host_gauges_snapshot)
        .await
        .unwrap_or_else(|_| host_gauges_snapshot());
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(
            serde_json::json!({
                "ok": true,
                "ts": now_unix(),
                "minimalist": minimalist,
                "cpu": {
                    "percent": snap.cpu_percent,
                    "detail": snap.cpu_detail,
                    "stroke": gauge_stroke_for_usage(snap.cpu_percent.unwrap_or(0)),
                },
                "ram": {
                    "percent": snap.ram_percent,
                    "detail": snap.ram_detail,
                    "stroke": gauge_stroke_for_usage(snap.ram_percent.unwrap_or(0)),
                },
                "disk": {
                    "percent": snap.disk_percent,
                    "detail": snap.disk_detail,
                    "stroke": gauge_stroke_for_usage(snap.disk_percent.unwrap_or(0)),
                },
            })
            .to_string(),
        )
}

/// Client poller for dashboard `.resource-grid` gauges.
pub fn dashboard_metrics_script(poll_ms: u64) -> String {
    format!(
        r#"
<script>
(function(){{
  var grid=document.querySelector('.resource-grid[data-live-metrics="1"]');
  if(!grid) return;
  var pollMs={poll_ms};
  var statusEl=document.getElementById('dash-metrics-status');
  var timer=null;
  var fail=0;
  var ctrl=null;
  var lastOk=0;

  function preferReduced(){{
    return window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }}
  function setStatus(msg, state){{
    if(!statusEl) return;
    statusEl.textContent=msg||'';
    statusEl.setAttribute('data-state', state||'ok');
  }}
  function label(v){{ return (v===null||v===undefined) ? 'n/a' : String(v); }}
  function applyGauge(card, percent, detail, stroke){{
    if(!card) return;
    var strong=card.querySelector('.gauge-copy strong');
    var span=card.querySelector('.gauge-copy span');
    var gauge=card.querySelector('.gauge');
    var circle=card.querySelector('circle.gauge-value');
    var pct=percent;
    if(strong){{
      strong.textContent=label(pct)+'%';
      if(stroke) strong.style.color=stroke;
    }}
    if(span && detail) span.textContent=detail;
    if(gauge){{
      var name=(card.querySelector('h2')||{{}}).textContent||'Usage';
      gauge.setAttribute('aria-label', name+': '+label(pct)+'%');
    }}
    if(circle && pct!==null && pct!==undefined){{
      var r=42;
      var c=2*Math.PI*r;
      var offset=c*(1-(Math.max(0,Math.min(100,pct))/100));
      circle.setAttribute('stroke-dasharray', String(c));
      circle.setAttribute('stroke-dashoffset', String(offset));
      if(stroke) circle.setAttribute('stroke', stroke);
      if(!preferReduced()){{
        circle.style.transition='stroke-dashoffset .4s ease, stroke .2s ease';
      }}
    }}
  }}
  function cards(){{
    return Array.prototype.slice.call(grid.querySelectorAll('.resource-card'));
  }}
  async function tick(){{
    if(document.hidden){{
      schedule(Math.max(pollMs, 15000));
      return;
    }}
    if(ctrl) try{{ ctrl.abort(); }}catch(_e){{}}
    ctrl=new AbortController();
    try{{
      var res=await fetch('/api/dashboard/metrics',{{
        credentials:'same-origin',
        headers:{{'Accept':'application/json'}},
        signal:ctrl.signal
      }});
      if(res.status===401){{
        setStatus('Session expired. Sign in again to refresh metrics.', 'stale');
        return;
      }}
      var data=await res.json().catch(function(){{return null;}});
      if(!res.ok || !data || !data.ok){{
        fail++;
        setStatus('Metrics update paused (retrying).', 'stale');
        schedule(Math.min(60000, pollMs * Math.pow(2, Math.min(fail, 4))));
        return;
      }}
      if(data.minimalist){{
        setStatus('Minimalist mode: refresh the page to update gauges.', 'ok');
        return;
      }}
      fail=0;
      lastOk=Date.now();
      var list=cards();
      applyGauge(list[0], data.cpu && data.cpu.percent, data.cpu && data.cpu.detail, data.cpu && data.cpu.stroke);
      applyGauge(list[1], data.ram && data.ram.percent, data.ram && data.ram.detail, data.ram && data.ram.stroke);
      applyGauge(list[2], data.disk && data.disk.percent, data.disk && data.disk.detail, data.disk && data.disk.stroke);
      var t=new Date();
      var stamp=('0'+t.getDate()).slice(-2)+'/'+('0'+(t.getMonth()+1)).slice(-2)+'/'+t.getFullYear()
        +' '+('0'+t.getHours()).slice(-2)+':'+('0'+t.getMinutes()).slice(-2)+':'+('0'+t.getSeconds()).slice(-2);
      setStatus('Updated '+stamp, 'ok');
      schedule(pollMs);
    }}catch(e){{
      if(e && e.name==='AbortError') return;
      fail++;
      setStatus('Connection lost. Reconnecting…', 'stale');
      schedule(Math.min(60000, pollMs * Math.pow(2, Math.min(fail, 4))));
    }}
  }}
  function schedule(ms){{
    if(timer) clearTimeout(timer);
    timer=setTimeout(tick, ms);
  }}
  document.addEventListener('visibilitychange', function(){{
    if(!document.hidden){{
      if(lastOk && (Date.now()-lastOk)>pollMs*2){{
        setStatus('Refreshing stale metrics…', 'stale');
      }}
      tick();
    }}
  }});
  window.addEventListener('cpn:spa-navigated', function(){{
    grid=document.querySelector('.resource-grid[data-live-metrics="1"]');
    statusEl=document.getElementById('dash-metrics-status');
    if(grid) tick();
  }});
  if(pollMs>0){{
    setStatus('Connecting live metrics…', 'ok');
    schedule(1200);
  }}else{{
    setStatus('Minimalist mode: refresh the page to update gauges.', 'ok');
  }}
}})();
</script>
"#,
        poll_ms = poll_ms,
    )
}
