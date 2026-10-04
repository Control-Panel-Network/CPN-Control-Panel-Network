//! Shared Markdown/HTML editor widgets for Settings (toolbar, Preview, View HTML source).

fn field_html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Shared Markdown preview endpoint (owner-only POST).
pub const MARKDOWN_PREVIEW_PATH: &str = "/settings/markdown/preview";

/// Toolbar + preview script/CSS matching rawdata-style Markdown editors.
pub fn markdown_toolbar_assets() -> &'static str {
    r#"
<style>
.cpn-md-wrap { margin:0 0 12px; }
.cpn-md-label {
  display:block; margin:0 0 6px; font-size:11px; letter-spacing:.04em;
  text-transform:uppercase; color:var(--muted,#94a3b8); font-weight:600;
}
.cpn-md-box {
  border:1px solid var(--hairline,#334155); border-radius:8px; background:var(--canvas,#0f172a);
  overflow:hidden;
}
.cpn-md-toolbar {
  display:flex; flex-wrap:wrap; gap:6px; padding:8px; border-bottom:1px solid var(--hairline,#334155);
  background:rgba(15,23,42,.55);
}
.cpn-md-btn {
  min-width:34px; height:32px; padding:0 8px; border:1px solid #475569; border-radius:6px;
  background:#1e293b; color:#e2e8f0; font-size:12px; font-weight:700; cursor:pointer; line-height:1;
}
.cpn-md-btn:hover { border-color:#38bdf8; color:#7dd3fc; }
.cpn-md-btn.is-active { background:#334155; border-color:#94a3b8; }
.cpn-md-btn.is-italic { font-style:italic; }
.cpn-md-box textarea {
  display:block; width:100%; min-height:140px; box-sizing:border-box; margin:0; padding:12px;
  border:0; resize:vertical; background:transparent; color:var(--ink,#e2e8f0); font:inherit;
}
.cpn-md-preview {
  display:none; min-height:140px; max-height:360px; overflow:auto; padding:12px;
  color:var(--ink,#e2e8f0); font-size:14px; line-height:1.5;
}
.cpn-md-preview.is-open { display:block; }
.cpn-md-preview iframe.cpn-md-preview-frame {
  display:none; width:100%; min-height:280px; border:0; border-radius:6px; background:#fff;
}
.cpn-md-preview iframe.cpn-md-preview-frame.is-open { display:block; }
.cpn-md-preview.is-open + textarea, .cpn-md-box.is-previewing textarea { display:none; }
.cpn-md-box.is-html-source textarea.cpn-md-html-source { display:block; }
.cpn-md-box.is-html-source textarea:not(.cpn-md-html-source),
.cpn-md-box.is-html-source .cpn-md-preview { display:none !important; }
.cpn-md-html-source {
  display:none; width:100%; min-height:140px; box-sizing:border-box; margin:0; padding:12px;
  border:0; resize:vertical; background:#020617; color:#cbd5e1; font-family:ui-monospace,monospace;
  font-size:12px; line-height:1.45;
}
.cpn-md-actions { display:flex; flex-wrap:wrap; justify-content:flex-end; gap:8px; margin-top:8px; }
.cpn-md-preview-btn {
  display:inline-flex; align-items:center; gap:6px; padding:8px 14px; border-radius:8px;
  border:1px solid #475569; background:#334155; color:#f1f5f9; font:inherit; font-size:13px;
  font-weight:600; cursor:pointer;
}
.cpn-md-preview-btn:hover { border-color:#94a3b8; }
.cpn-md-preview h1,.cpn-md-preview h2,.cpn-md-preview h3 { margin:0.6em 0 0.35em; }
.cpn-md-preview p { margin:0.4em 0; }
.cpn-md-preview code { font-family:ui-monospace,monospace; font-size:0.92em; }
.cpn-md-preview pre { padding:10px; overflow:auto; border-radius:6px; background:#020617; }
.cpn-md-preview table { border-collapse:collapse; width:100%; margin:0.5em 0; }
.cpn-md-preview td,.cpn-md-preview th { border:1px solid #475569; padding:6px 8px; }
</style>
<script>
(function(){
  function wrap(ta, before, after, ph){
    var s=ta.selectionStart||0, e=ta.selectionEnd||0, v=ta.value||'';
    var sel=v.substring(s,e); var ins=sel.length?sel:(ph||'');
    ta.value=v.substring(0,s)+before+ins+after+v.substring(e);
    ta.focus(); ta.setSelectionRange(s+before.length, s+before.length+ins.length);
  }
  function prefix(ta, pfx){
    var s=ta.selectionStart||0, e=ta.selectionEnd||0, v=ta.value||'';
    var bs=v.lastIndexOf('\n', Math.max(0,s-1))+1;
    var be=v.indexOf('\n', e); if(be<0) be=v.length;
    var block=v.substring(bs,be).split('\n').map(function(l){return pfx+l;}).join('\n');
    ta.value=v.substring(0,bs)+block+v.substring(be);
    ta.focus(); ta.setSelectionRange(bs, bs+block.length);
  }
  function insert(ta, block){
    var s=ta.selectionStart||0, v=ta.value||'';
    ta.value=v.substring(0,s)+block+v.substring(s);
    var p=s+block.length; ta.focus(); ta.setSelectionRange(p,p);
  }
  function previewUrl(root){
    return root.getAttribute('data-preview-url') || '/settings/markdown/preview';
  }
  function closeModes(box, prevBtn, srcBtn){
    var prev=box.querySelector('.cpn-md-preview');
    var frame=box.querySelector('.cpn-md-preview-frame');
    if(prev){
      prev.classList.remove('is-open');
      Array.from(prev.childNodes).forEach(function(n){
        if(!frame || n!==frame) prev.removeChild(n);
      });
    }
    if(frame){ frame.classList.remove('is-open'); frame.removeAttribute('srcdoc'); }
    box.classList.remove('is-previewing','is-html-source');
    if(prevBtn){ prevBtn.classList.remove('is-active'); prevBtn.setAttribute('aria-pressed','false'); }
    if(srcBtn){ srcBtn.classList.remove('is-active'); srcBtn.setAttribute('aria-pressed','false'); }
  }
  function togglePreview(root, box, ta, btn, srcBtn){
    var prev=box.querySelector('.cpn-md-preview');
    if(!prev) return;
    var open=prev.classList.contains('is-open') || box.classList.contains('is-previewing');
    if(open){
      closeModes(box, btn, srcBtn);
      return;
    }
    closeModes(box, btn, srcBtn);
    var isTemplate=root.getAttribute('data-cpn-html-template')==='1';
    var bodyKey=isTemplate?'html':'markdown';
    var payload=ta.value||'';
    fetch(previewUrl(root), {
      method:'POST',
      headers:{'Content-Type':'application/x-www-form-urlencoded'},
      body:bodyKey+'='+encodeURIComponent(payload),
      credentials:'same-origin'
    }).then(function(r){ return r.text(); }).then(function(html){
      if(isTemplate){
        var frame=box.querySelector('.cpn-md-preview-frame');
        if(!frame){
          frame=document.createElement('iframe');
          frame.className='cpn-md-preview-frame';
          frame.setAttribute('title','Template preview');
          frame.setAttribute('sandbox','');
          prev.appendChild(frame);
        }
        Array.from(prev.childNodes).forEach(function(n){
          if(n!==frame) prev.removeChild(n);
        });
        frame.setAttribute('srcdoc', html);
        frame.classList.add('is-open');
      } else {
        prev.innerHTML='';
        var slot=document.createElement('div');
        slot.innerHTML=html;
        prev.appendChild(slot);
      }
      prev.classList.add('is-open'); box.classList.add('is-previewing');
      btn.classList.add('is-active'); btn.setAttribute('aria-pressed','true');
    }).catch(function(){
      prev.textContent=payload;
      prev.classList.add('is-open'); box.classList.add('is-previewing');
    });
  }
  function toggleHtmlSource(root, box, ta, btn, prevBtn){
    var srcTa=box.querySelector('.cpn-md-html-source');
    if(!srcTa) return;
    var open=box.classList.contains('is-html-source');
    if(open){
      closeModes(box, prevBtn, btn);
      return;
    }
    closeModes(box, prevBtn, btn);
    var isTemplate=root.getAttribute('data-cpn-html-template')==='1';
    if(isTemplate){
      srcTa.value=ta.value||'';
      box.classList.add('is-html-source');
      btn.classList.add('is-active'); btn.setAttribute('aria-pressed','true');
      return;
    }
    fetch(previewUrl(root), {
      method:'POST',
      headers:{'Content-Type':'application/x-www-form-urlencoded'},
      body:'markdown='+encodeURIComponent(ta.value||''),
      credentials:'same-origin'
    }).then(function(r){ return r.text(); }).then(function(html){
      srcTa.value=html;
      box.classList.add('is-html-source');
      btn.classList.add('is-active'); btn.setAttribute('aria-pressed','true');
    }).catch(function(){
      srcTa.value='(Could not render HTML preview)';
      box.classList.add('is-html-source');
    });
  }
  function bind(root){
    var box=root.querySelector('.cpn-md-box')||root;
    var ta=box.querySelector('textarea:not(.cpn-md-html-source)');
    if(!ta) return;
    var prevBtn=root.querySelector('[data-md-action="preview"]');
    var srcBtn=root.querySelector('[data-md-action="html-source"]');
    root.querySelectorAll('[data-md-action]').forEach(function(btn){
      btn.addEventListener('click', function(ev){
        ev.preventDefault();
        var a=btn.getAttribute('data-md-action');
        if(a==='bold') wrap(ta,'**','**','bold');
        else if(a==='italic') wrap(ta,'*','*','italic');
        else if(a==='code') wrap(ta,'`','`','code');
        else if(a==='codeblock') insert(ta,'\n```\ncode\n```\n');
        else if(a==='link'){ var u=prompt('URL (https://...)','https://'); if(u) wrap(ta,'[',']('+u+')','link text'); }
        else if(a==='image'){ var u=prompt('Image URL (https://...)','https://'); if(u) insert(ta,'![alt]('+u+')'); }
        else if(a==='quote') prefix(ta,'> ');
        else if(a==='ul') prefix(ta,'- ');
        else if(a==='ol') prefix(ta,'1. ');
        else if(a==='heading') prefix(ta,'## ');
        else if(a==='table') insert(ta,'| Column | Column |\n| --- | --- |\n| Cell | Cell |\n');
        else if(a==='hr') insert(ta,'\n---\n');
        else if(a==='preview') togglePreview(root, box, ta, btn, srcBtn);
        else if(a==='html-source') toggleHtmlSource(root, box, ta, btn, prevBtn);
      });
    });
  }
  function bindAll(){
    document.querySelectorAll('[data-cpn-md]').forEach(bind);
  }
  // Assets are often emitted before the editor markup in the same response.
  // Wait for DOM parse so toolbar / Preview / HTML source buttons bind.
  if(document.readyState==='loading'){
    document.addEventListener('DOMContentLoaded', bindAll);
  } else {
    bindAll();
  }
})();
</script>
"#
}

/// Build a labeled Markdown editor block for a form field.
pub fn markdown_editor_field(label: &str, name: &str, value: &str, placeholder: &str) -> String {
    markdown_editor_field_with_preview(label, name, value, placeholder, MARKDOWN_PREVIEW_PATH)
}

/// Markdown editor with a custom preview POST path.
pub fn markdown_editor_field_with_preview(
    label: &str,
    name: &str,
    value: &str,
    placeholder: &str,
    preview_path: &str,
) -> String {
    format!(
        r#"<div class="cpn-md-wrap" data-cpn-md data-preview-url="{preview_url}">
  <span class="cpn-md-label">{label}</span>
  <div class="cpn-md-box">
    <div class="cpn-md-toolbar" role="toolbar" aria-label="Markdown formatting">
      <button type="button" class="cpn-md-btn" data-md-action="bold" title="Bold">B</button>
      <button type="button" class="cpn-md-btn is-italic" data-md-action="italic" title="Italic">I</button>
      <button type="button" class="cpn-md-btn" data-md-action="code" title="Inline code">&lt;/&gt;</button>
      <button type="button" class="cpn-md-btn" data-md-action="codeblock" title="Code block">{{ }}</button>
      <button type="button" class="cpn-md-btn" data-md-action="link" title="Link">Link</button>
      <button type="button" class="cpn-md-btn" data-md-action="image" title="Image">Img</button>
      <button type="button" class="cpn-md-btn" data-md-action="quote" title="Blockquote">&ldquo;</button>
      <button type="button" class="cpn-md-btn" data-md-action="ul" title="Bulleted list">UL</button>
      <button type="button" class="cpn-md-btn" data-md-action="ol" title="Numbered list">OL</button>
      <button type="button" class="cpn-md-btn" data-md-action="heading" title="Heading">H</button>
      <button type="button" class="cpn-md-btn" data-md-action="table" title="Table">Tbl</button>
      <button type="button" class="cpn-md-btn" data-md-action="hr" title="Horizontal rule">-</button>
    </div>
    <div class="cpn-md-preview" aria-live="polite"></div>
    <textarea class="cpn-md-html-source" readonly aria-label="Rendered HTML source"></textarea>
    <textarea id="{name}" name="{name}" rows="8" placeholder="{ph}">{value}</textarea>
  </div>
  <div class="cpn-md-actions">
    <button type="button" class="cpn-md-preview-btn" data-md-action="preview" aria-pressed="false">
      <span aria-hidden="true">&#128065;</span> Preview
    </button>
    <button type="button" class="cpn-md-preview-btn" data-md-action="html-source" aria-pressed="false">
      View HTML source
    </button>
  </div>
</div>"#,
        label = field_html_escape(label),
        name = field_html_escape(name),
        value = field_html_escape(value),
        ph = field_html_escape(placeholder),
        preview_url = field_html_escape(preview_path),
    )
}

/// Full HTML document editor (site-ready templates): preview + view source, no Markdown toolbar.
pub fn html_template_editor_field(
    label: &str,
    name: &str,
    value: &str,
    placeholder: &str,
    preview_path: &str,
) -> String {
    format!(
        r#"<div class="cpn-md-wrap" data-cpn-md data-cpn-html-template="1" data-preview-url="{preview_url}">
  <span class="cpn-md-label">{label}</span>
  <div class="cpn-md-box">
    <div class="cpn-md-preview" aria-live="polite"></div>
    <textarea class="cpn-md-html-source" readonly aria-label="HTML source (read-only view)"></textarea>
    <textarea id="{name}" name="{name}" rows="14" placeholder="{ph}" style="font-family:ui-monospace,monospace;font-size:13px;">{value}</textarea>
  </div>
  <div class="cpn-md-actions">
    <button type="button" class="cpn-md-preview-btn" data-md-action="preview" aria-pressed="false">
      <span aria-hidden="true">&#128065;</span> Preview
    </button>
    <button type="button" class="cpn-md-preview-btn" data-md-action="html-source" aria-pressed="false">
      View HTML source
    </button>
  </div>
</div>"#,
        label = field_html_escape(label),
        name = field_html_escape(name),
        value = field_html_escape(value),
        ph = field_html_escape(placeholder),
        preview_url = field_html_escape(preview_path),
    )
}
