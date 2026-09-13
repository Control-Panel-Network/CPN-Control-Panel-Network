//! Root File Manager UI (classic hosting file manager layout, CPN branding).

use crate::panel_brand::brand_mark_svg;
use crate::panel_hub_http::urlencoding_simple;
use crate::panel_hubs::{feature_shell, notice_block};
use crate::panel_ops_files::files_csrf_token;
use crate::panel_ops_path::{list_dir, resolve_under_allowlist};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn files_page(
    username: &str,
    path_q: &str,
    notice: Option<&str>,
    error: Option<&str>,
    edit_path: Option<&str>,
    edit_content: Option<&str>,
) -> String {
    let csrf = files_csrf_token(username);
    let resolved = resolve_under_allowlist(path_q);
    let body = match resolved {
        Ok(path) => match list_dir(&path) {
            Ok(entries) => {
                let path_s = path.display().to_string();
                let parent_href = path
                    .parent()
                    .map(|p| {
                        format!(
                            "/server/files?path={}",
                            urlencoding_simple(&p.display().to_string())
                        )
                    })
                    .unwrap_or_else(|| "/server/files?path=%2F".into());
                let mut rows = String::new();
                for ent in &entries {
                    let child = path.join(&ent.basename);
                    let child_s = child.display().to_string();
                    let name_cell = if ent.is_dir {
                        format!(
                            r#"<a href="/server/files?path={p}">{label}</a>"#,
                            p = urlencoding_simple(&child_s),
                            label = html_escape(&ent.label),
                        )
                    } else {
                        html_escape(&ent.label)
                    };
                    let size_kb = if ent.is_dir {
                        "-".into()
                    } else {
                        format!("{:.1}", ent.size as f64 / 1024.0)
                    };
                    rows.push_str(&format!(
                        r#"<tr>
                      <td><input type="checkbox" class="fm-check" value="{base}"></td>
                      <td class="fm-name">{name}</td>
                      <td>{size}</td>
                      <td>{mtime}</td>
                      <td><code>{mode}</code></td>
                    </tr>"#,
                        base = html_escape(&ent.basename),
                        name = name_cell,
                        size = size_kb,
                        mtime = html_escape(&ent.mtime_label),
                        mode = html_escape(&ent.mode_label),
                    ));
                }
                let tree = build_tree_html(&path_s);
                let editor = edit_modal(edit_path, edit_content, &csrf, &path_s);
                let notices = format!(
                    "{}{}",
                    notice_block("ok", notice),
                    notice_block("error", error)
                );
                format!(
                    r#"{styles}
{risk}
{notices}
<div class="fm-root" id="fm-root" data-path="{path_esc}" data-csrf="{csrf}">
  <div class="fm-brandbar">
    <span class="fm-logo">{logo}</span>
    <strong>CPN Panel</strong>
    <span class="fm-brand-sub">Root File Manager</span>
  </div>
  <div class="fm-toolbar" role="toolbar" aria-label="File actions">
    <label class="fm-tool"><input type="file" id="fm-upload" hidden multiple>Upload</label>
    <button type="button" class="fm-tool" data-op="create">New File</button>
    <button type="button" class="fm-tool" data-op="mkdir">New Folder</button>
    <button type="button" class="fm-tool" data-op="delete">Delete</button>
    <button type="button" class="fm-tool" data-op="copy">Copy</button>
    <button type="button" class="fm-tool" data-op="move">Move</button>
    <button type="button" class="fm-tool" data-op="rename">Rename</button>
    <button type="button" class="fm-tool" data-op="edit">Edit</button>
    <button type="button" class="fm-tool" data-op="compress">Compress</button>
    <button type="button" class="fm-tool" data-op="extract">Extract</button>
  </div>
  <div class="fm-navrow">
    <a class="fm-navbtn" href="/server/files?path=%2F">Home</a>
    <a class="fm-navbtn" href="{back}">Back</a>
    <a class="fm-navbtn" href="/server/files?path={path_enc}">Refresh</a>
    <button type="button" class="fm-navbtn" id="fm-select-all">Select All</button>
    <button type="button" class="fm-navbtn" id="fm-unselect-all">UnSelect All</button>
    <button type="button" class="fm-navbtn fm-tree-toggle" id="fm-tree-toggle" aria-expanded="true">Tree</button>
  </div>
  <div class="fm-body">
    <aside class="fm-tree" id="fm-tree">
      <div class="fm-tree-title">Current Path</div>
      <code class="fm-cwd">{path_esc}</code>
      {tree}
    </aside>
    <div class="fm-main">
      <form method="get" action="/server/files" class="fm-pathform">
        <label for="fm-path">Path</label>
        <input id="fm-path" name="path" type="text" value="{path_esc}">
        <button type="submit" class="btn-primary">Open</button>
      </form>
      <form id="fm-op" method="post" action="/server/files/op" class="fm-hidden-form">
        <input type="hidden" name="csrf" value="{csrf}">
        <input type="hidden" name="path" value="{path_esc}">
        <input type="hidden" name="op" id="fm-op-field" value="">
        <input type="hidden" name="dest" id="fm-dest-field" value="">
        <input type="hidden" name="new_name" id="fm-new-name" value="">
        <input type="hidden" name="archive_name" id="fm-archive-name" value="">
        <input type="hidden" name="names_csv" id="fm-names-csv" value="">
      </form>
      <div class="table-wrap fm-table-wrap">
        <table class="data-table fm-table">
          <thead><tr><th></th><th>File Name</th><th>Size (KB)</th><th>Last Modified</th><th>Permissions</th></tr></thead>
          <tbody>{rows}</tbody>
        </table>
      </div>
    </div>
  </div>
</div>
{editor}
{script}"#,
                    styles = fm_styles(),
                    risk = r#"<p class="muted">Admin-only full filesystem access. Path traversal is blocked; protected system paths refuse delete/overwrite. Prefer site jails for routine hosting work.</p>"#,
                    notices = notices,
                    path_esc = html_escape(&path_s),
                    path_enc = urlencoding_simple(&path_s),
                    csrf = html_escape(&csrf),
                    logo = brand_mark_svg(),
                    back = parent_href,
                    tree = tree,
                    rows = rows,
                    editor = editor,
                    script = fm_script(),
                )
            }
            Err(err) => format!(
                "{}{}",
                notice_block("error", Some(&err)),
                notice_block("ok", notice)
            ),
        },
        Err(err) => format!(
            "{}{}",
            notice_block("error", Some(&err)),
            notice_block("ok", notice)
        ),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Root File Manager", None),
        ],
        "Root File Manager",
        "Browse and manage the server filesystem from CPN Panel.",
        &body,
        None,
        None,
    )
}

fn edit_modal(edit_path: Option<&str>, content: Option<&str>, csrf: &str, cwd: &str) -> String {
    let Some(path) = edit_path.filter(|p| !p.is_empty()) else {
        return String::new();
    };
    format!(
        r#"<div class="fm-modal" role="dialog" aria-modal="true" aria-label="Edit file">
  <form method="post" action="/server/files/op" class="fm-modal-card">
    <h3>Edit {name}</h3>
    <input type="hidden" name="csrf" value="{csrf}">
    <input type="hidden" name="path" value="{cwd}">
    <input type="hidden" name="op" value="write">
    <input type="hidden" name="new_name" value="{path}">
    <textarea name="content" rows="18" class="fm-editor">{body}</textarea>
    <div class="fm-modal-actions">
      <button type="submit" class="btn-primary">Save</button>
      <a class="btn-secondary" href="/server/files?path={cwd_enc}">Cancel</a>
    </div>
  </form>
</div>"#,
        name = html_escape(path),
        csrf = html_escape(csrf),
        cwd = html_escape(cwd),
        cwd_enc = urlencoding_simple(cwd),
        path = html_escape(path),
        body = html_escape(content.unwrap_or("")),
    )
}

fn build_tree_html(current: &str) -> String {
    let roots = match list_dir(std::path::Path::new("/")) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    let mut out = String::from(r#"<ul class="fm-tree-list"><li><a href="/server/files?path=%2F">/</a><ul>"#);
    for ent in roots.into_iter().filter(|e| e.is_dir) {
        let p = format!("/{}", ent.basename);
        let open = current == p || current.starts_with(&(p.clone() + "/"));
        let kids = if open {
            match list_dir(std::path::Path::new(&p)) {
                Ok(entries) => {
                    let mut inner = String::from("<ul>");
                    for child in entries.into_iter().filter(|e| e.is_dir).take(40) {
                        let cp = format!("{}/{}", p.trim_end_matches('/'), child.basename);
                        inner.push_str(&format!(
                            r#"<li><a href="/server/files?path={enc}">{name}</a></li>"#,
                            enc = urlencoding_simple(&cp),
                            name = html_escape(&child.basename),
                        ));
                    }
                    inner.push_str("</ul>");
                    inner
                }
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };
        out.push_str(&format!(
            r#"<li><a href="/server/files?path={enc}">{name}</a>{kids}</li>"#,
            enc = urlencoding_simple(&p),
            name = html_escape(&ent.basename),
            kids = kids,
        ));
    }
    out.push_str("</ul></li></ul>");
    out
}

fn fm_styles() -> &'static str {
    r#"<style>
.fm-root{display:flex;flex-direction:column;gap:10px;margin-top:8px;}
.fm-brandbar{display:flex;align-items:center;gap:10px;padding:10px 12px;background:var(--surface-soft,#161922);border:1px solid var(--hairline,#2a2f3a);border-radius:8px;}
.fm-brandbar .cpn-brand-mark{width:28px;height:28px;}
.fm-brand-sub{color:var(--muted);font-size:13px;margin-left:auto;}
.fm-toolbar{display:flex;flex-wrap:wrap;gap:6px;padding:8px;background:#0f172a;border-radius:8px;}
.fm-tool,.fm-navbtn{appearance:none;border:1px solid #334155;background:#1e293b;color:#e2e8f0;padding:6px 10px;border-radius:6px;font:inherit;font-size:13px;cursor:pointer;text-decoration:none;display:inline-flex;align-items:center;}
.fm-tool:hover,.fm-navbtn:hover{border-color:var(--blue,#006CFA);color:#fff;}
.fm-navrow{display:flex;flex-wrap:wrap;gap:6px;align-items:center;}
.fm-body{display:grid;grid-template-columns:240px 1fr;gap:12px;min-height:420px;}
.fm-tree{border:1px solid var(--hairline,#2a2f3a);border-radius:8px;padding:10px;background:var(--surface-soft,#161922);overflow:auto;max-height:70vh;}
.fm-tree-title{font-weight:600;margin-bottom:6px;}
.fm-cwd{display:block;font-size:12px;word-break:break-all;margin-bottom:10px;color:var(--muted);}
.fm-tree-list{list-style:none;padding-left:0;margin:0;font-size:13px;}
.fm-tree-list ul{list-style:none;padding-left:14px;margin:4px 0;}
.fm-tree-list a{color:inherit;text-decoration:none;}
.fm-tree-list a:hover{color:var(--blue,#006CFA);}
.fm-main{min-width:0;}
.fm-pathform{display:flex;flex-wrap:wrap;gap:8px;align-items:end;margin-bottom:10px;}
.fm-pathform input{flex:1;min-width:180px;}
.fm-table th:first-child,.fm-table td:first-child{width:2rem;}
.fm-name{word-break:break-all;}
.fm-hidden-form{display:none;}
.fm-modal{position:fixed;inset:0;background:rgba(0,0,0,.55);display:flex;align-items:center;justify-content:center;z-index:80;padding:16px;}
.fm-modal-card{background:var(--surface,#12141a);border:1px solid var(--hairline,#2a2f3a);border-radius:10px;padding:16px;width:min(920px,100%);max-height:90vh;overflow:auto;}
.fm-editor{width:100%;font-family:ui-monospace,Consolas,monospace;font-size:13px;background:#0b1220;color:#e2e8f0;border:1px solid #334155;border-radius:6px;padding:10px;}
.fm-modal-actions{display:flex;gap:8px;margin-top:12px;}
@media (max-width:860px){
  .fm-body{grid-template-columns:1fr;}
  .fm-tree.fm-tree-collapsed{display:none;}
  .fm-tree{max-height:220px;}
}
</style>"#
}

fn fm_script() -> &'static str {
    r#"<script>
(function(){
  var root=document.getElementById('fm-root');
  if(!root) return;
  var csrf=root.getAttribute('data-csrf')||'';
  var path=root.getAttribute('data-path')||'/';
  function selected(){
    return Array.prototype.map.call(document.querySelectorAll('.fm-check:checked'), function(el){return el.value;});
  }
  function submitOp(op, extra){
    document.getElementById('fm-op-field').value=op;
    document.getElementById('fm-dest-field').value=(extra&&extra.dest)||'';
    document.getElementById('fm-new-name').value=(extra&&extra.new_name)||'';
    document.getElementById('fm-archive-name').value=(extra&&extra.archive_name)||'';
    document.getElementById('fm-names-csv').value=((extra&&extra.names)||selected()).join('\n');
    document.getElementById('fm-op').submit();
  }
  document.getElementById('fm-select-all').addEventListener('click', function(){
    document.querySelectorAll('.fm-check').forEach(function(c){c.checked=true;});
  });
  document.getElementById('fm-unselect-all').addEventListener('click', function(){
    document.querySelectorAll('.fm-check').forEach(function(c){c.checked=false;});
  });
  var tree=document.getElementById('fm-tree');
  document.getElementById('fm-tree-toggle').addEventListener('click', function(){
    tree.classList.toggle('fm-tree-collapsed');
    this.setAttribute('aria-expanded', tree.classList.contains('fm-tree-collapsed')?'false':'true');
  });
  document.getElementById('fm-upload').addEventListener('change', function(){
    var files=this.files; if(!files||!files.length) return;
    var i=0;
    function next(){
      if(i>=files.length){ location.href='/server/files?path='+encodeURIComponent(path)+'&notice='+encodeURIComponent('Upload finished'); return; }
      var f=files[i++];
      var reader=new FileReader();
      reader.onload=function(){
        var b64=String(reader.result||'');
        var body=new URLSearchParams();
        body.set('csrf', csrf);
        body.set('path', path);
        body.set('filename', f.name);
        body.set('file_b64', b64);
        fetch('/server/files/upload', {
          method:'POST',
          credentials:'same-origin',
          headers:{'Content-Type':'application/x-www-form-urlencoded'},
          body:body.toString(),
          redirect:'follow'
        }).then(function(){ next(); }).catch(function(){ alert('Upload failed for '+f.name); });
      };
      reader.readAsDataURL(f);
    }
    next();
  });
  root.querySelectorAll('.fm-tool[data-op]').forEach(function(btn){
    btn.addEventListener('click', function(){
      var op=btn.getAttribute('data-op');
      var names=selected();
      if(op==='mkdir'){
        var n=prompt('New folder name'); if(!n) return;
        submitOp('mkdir', {new_name:n, names:[]});
        return;
      }
      if(op==='create'){
        var n=prompt('New file name'); if(!n) return;
        submitOp('create', {new_name:n, names:[]});
        return;
      }
      if(op==='delete'){
        if(!names.length){ alert('Select one or more items'); return; }
        if(!confirm('Delete '+names.length+' item(s)? This cannot be undone.')) return;
        submitOp('delete', {names:names});
        return;
      }
      if(op==='rename'){
        if(names.length!==1){ alert('Select exactly one item to rename'); return; }
        var n=prompt('New name', names[0]); if(!n) return;
        submitOp('rename', {names:names, new_name:n});
        return;
      }
      if(op==='copy'||op==='move'){
        if(!names.length){ alert('Select one or more items'); return; }
        var dest=prompt('Destination directory (absolute path)', path); if(!dest) return;
        submitOp(op, {names:names, dest:dest});
        return;
      }
      if(op==='edit'){
        if(names.length!==1){ alert('Select exactly one file to edit'); return; }
        location.href='/server/files?path='+encodeURIComponent(path)+'&edit='+encodeURIComponent(path.replace(/\\/$/,'')+'/'+names[0]);
        return;
      }
      if(op==='compress'){
        if(!names.length){ alert('Select one or more items'); return; }
        var n=prompt('Archive name (.zip or .tar.gz)', 'archive.tar.gz'); if(!n) return;
        submitOp('compress', {names:names, archive_name:n});
        return;
      }
      if(op==='extract'){
        if(names.length!==1){ alert('Select one archive to extract'); return; }
        submitOp('extract', {names:names});
        return;
      }
    });
  });
})();
</script>"#
}
