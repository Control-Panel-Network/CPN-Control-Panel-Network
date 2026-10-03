(function(){
  window.cpnFmReady=true;
  var root=document.getElementById('fm-root');
  if(!root) return;
  var csrf=root.getAttribute('data-csrf')||'';
  var path=root.getAttribute('data-path')||'/';
  var base=root.getAttribute('data-base')||'/server/files';
  var opUrl=root.getAttribute('data-op')||(base+'/op');
  var uploadUrl=root.getAttribute('data-upload')||(base+'/upload');
  var listUrl=root.getAttribute('data-list')||(base+'/list');
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
  function esc(s){
    return String(s==null?'':s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/"/g,'&quot;');
  }
  function showStatus(msg, isErr){
    var el=document.getElementById('fm-list-status');
    if(!el) return;
    if(!msg){ el.hidden=true; el.textContent=''; return; }
    el.hidden=false;
    el.className=isErr?'panel-notice error':'muted';
    el.textContent=msg;
  }
  function childPath(name){
    if(path==='/'||path==='') return '/'+name;
    return path.replace(/\/$/,'')+'/'+name;
  }
  function renderRows(data){
    var tb=document.getElementById('fm-rows');
    if(!tb) return;
    var entries=data.entries||[];
    if(!entries.length){
      tb.innerHTML='<tr><td colspan="5">This directory is empty.</td></tr>';
      return;
    }
    var html='';
    for(var i=0;i<entries.length;i++){
      var ent=entries[i];
      var nameCell=ent.is_dir
        ? '<a href="'+pageHref(childPath(ent.basename))+'">'+esc(ent.label)+'</a>'
        : esc(ent.label);
      var size=ent.is_dir?'-':(Math.round((ent.size||0)/102.4)/10).toFixed(1);
      html+='<tr><td><input type="checkbox" class="fm-check" value="'+esc(ent.basename)+'"></td>';
      html+='<td class="fm-name">'+nameCell+'</td><td>'+size+'</td><td>'+esc(ent.mtime)+'</td><td><code>'+esc(ent.mode)+'</code></td></tr>';
    }
    tb.innerHTML=html;
  }
  function loadList(){
    var url=listUrl+(listUrl.indexOf('?')>=0?'&':'?')+qs+'path='+encodeURIComponent(path);
    fetch(url,{credentials:'same-origin',headers:{'Accept':'application/json'}})
      .then(function(res){
        if(res.status===503){
          return res.json().then(function(j){
            throw new Error((j&&j.error)||'Directory listing timed out. Open a narrower path.');
          }).catch(function(err){
            if(err&&err.message&&err.message.indexOf('timed out')>=0) throw err;
            throw new Error('Directory listing is busy. Reload in a few seconds.');
          });
        }
        if(!res.ok) throw new Error('Could not list this directory (HTTP '+res.status+').');
        return res.json();
      })
      .then(function(data){
        if(!data||data.ok===false){
          showStatus((data&&data.error)||'Could not list this directory.', true);
          document.getElementById('fm-rows').innerHTML='<tr><td colspan="5">Listing unavailable.</td></tr>';
          return;
        }
        var notes=[];
        if(data.timed_out) notes.push('Listing stopped after a time limit so the panel stays responsive.');
        if(data.truncated) notes.push('Showing a bounded set of entries. Open a narrower path for restore-sized directories.');
        showStatus(notes.join(' '), false);
        renderRows(data);
      })
      .catch(function(err){
        showStatus(err&&err.message?err.message:'Could not list this directory.', true);
        var tb=document.getElementById('fm-rows');
        if(tb) tb.innerHTML='<tr><td colspan="5">Listing failed.</td></tr>';
      });
  }
  var selAll=document.getElementById('fm-select-all');
  if(selAll) selAll.addEventListener('click', function(){
    document.querySelectorAll('.fm-check').forEach(function(c){c.checked=true;});
  });
  var unsel=document.getElementById('fm-unselect-all');
  if(unsel) unsel.addEventListener('click', function(){
    document.querySelectorAll('.fm-check').forEach(function(c){c.checked=false;});
  });
  var tree=document.getElementById('fm-tree');
  var tog=document.getElementById('fm-tree-toggle');
  if(tog&&tree) tog.addEventListener('click', function(){
    tree.classList.toggle('fm-tree-collapsed');
    this.setAttribute('aria-expanded', tree.classList.contains('fm-tree-collapsed')?'false':'true');
  });
  var up=document.getElementById('fm-upload');
  if(up) up.addEventListener('change', function(){
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
        location.href=pageHref(path,'edit='+encodeURIComponent(childPath(names[0])));
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
      }
    });
  });
  loadList();
})();
