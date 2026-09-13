//! Shared CSS/JS assets for File Manager UI.

pub fn fm_styles() -> &'static str {
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

pub fn fm_script() -> &'static str {
    r#"<script>
(function(){
  var root=document.getElementById('fm-root');
  if(!root) return;
  var csrf=root.getAttribute('data-csrf')||'';
  var path=root.getAttribute('data-path')||'/';
  var base=root.getAttribute('data-base')||'/server/files';
  var opUrl=root.getAttribute('data-op')||(base+'/op');
  var uploadUrl=root.getAttribute('data-upload')||(base+'/upload');
  var qs=root.getAttribute('data-qs')||'';
  function pageHref(p, extra){
    var u=base+'?'+qs+'path='+encodeURIComponent(p);
    if(extra) u+='&'+extra;
    return u;
  }
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
      if(i>=files.length){ location.href=pageHref(path,'notice='+encodeURIComponent('Upload finished')); return; }
      var f=files[i++];
      var reader=new FileReader();
      reader.onload=function(){
        var b64=String(reader.result||'');
        var body=new URLSearchParams();
        body.set('csrf', csrf);
        body.set('path', path);
        body.set('filename', f.name);
        body.set('file_b64', b64);
        if(qs){
          qs.split('&').forEach(function(pair){
            if(!pair) return;
            var kv=pair.split('=');
            if(kv[0]) body.set(decodeURIComponent(kv[0]), decodeURIComponent(kv[1]||''));
          });
        }
        fetch(uploadUrl, {
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
        var dest=prompt('Destination directory (absolute path inside jail)', path); if(!dest) return;
        submitOp(op, {names:names, dest:dest});
        return;
      }
      if(op==='edit'){
        if(names.length!==1){ alert('Select exactly one file to edit'); return; }
        location.href=pageHref(path,'edit='+encodeURIComponent(path.replace(/\/$/,'')+'/'+names[0]));
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
