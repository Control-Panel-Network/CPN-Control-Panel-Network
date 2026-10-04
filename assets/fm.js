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
  function goTo(href){
    // Prefer assign over setting location.href so URL navigation stays explicit.
    window.location.assign(href);
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
  function setRowsMessage(tb, message){
    while(tb.firstChild) tb.removeChild(tb.firstChild);
    var tr=document.createElement('tr');
    var td=document.createElement('td');
    td.colSpan=6;
    td.textContent=message;
    tr.appendChild(td);
    tb.appendChild(tr);
  }
  function renderRows(data){
    var tb=document.getElementById('fm-rows');
    if(!tb) return;
    var entries=data.entries||[];
    while(tb.firstChild) tb.removeChild(tb.firstChild);
    if(!entries.length){
      setRowsMessage(tb, 'This directory is empty.');
      return;
    }
    var folders=0, files=0;
    for(var i=0;i<entries.length;i++){
      var ent=entries[i];
      if(ent.is_dir) folders++; else files++;
      var tr=document.createElement('tr');
      tr.className=ent.is_dir?'fm-row fm-is-dir':'fm-row fm-is-file';
      var tdCheck=document.createElement('td');
      var cb=document.createElement('input');
      cb.type='checkbox';
      cb.className='fm-check';
      cb.value=String(ent.basename==null?'':ent.basename);
      tdCheck.appendChild(cb);
      var tdName=document.createElement('td');
      tdName.className='fm-name';
      var icon=document.createElement('span');
      icon.className=ent.is_dir?'fm-icon fm-icon-dir':'fm-icon fm-icon-file';
      icon.setAttribute('aria-hidden','true');
      tdName.appendChild(icon);
      var a=document.createElement('a');
      if(ent.is_dir){
        a.setAttribute('href', pageHref(childPath(ent.basename)));
        a.className='fm-link-dir';
      } else {
        a.setAttribute('href', pageHref(path,'edit='+encodeURIComponent(childPath(ent.basename))));
        a.className='fm-link-file';
        a.title='Open text editor';
      }
      a.textContent=String(ent.label==null?'':ent.label);
      tdName.appendChild(a);
      var tdType=document.createElement('td');
      tdType.className='fm-type';
      var badge=document.createElement('span');
      badge.className=ent.is_dir?'fm-badge fm-badge-dir':'fm-badge fm-badge-file';
      badge.textContent=ent.is_dir?'Folder':'File';
      tdType.appendChild(badge);
      var tdSize=document.createElement('td');
      tdSize.textContent=ent.is_dir?'-':(Math.round((ent.size||0)/102.4)/10).toFixed(1);
      var tdMtime=document.createElement('td');
      tdMtime.textContent=String(ent.mtime==null?'':ent.mtime);
      var tdMode=document.createElement('td');
      var code=document.createElement('code');
      code.textContent=String(ent.mode==null?'':ent.mode);
      tdMode.appendChild(code);
      tr.appendChild(tdCheck);
      tr.appendChild(tdName);
      tr.appendChild(tdType);
      tr.appendChild(tdSize);
      tr.appendChild(tdMtime);
      tr.appendChild(tdMode);
      tb.appendChild(tr);
    }
    var summary=folders+' folder'+(folders===1?'':'s')+', '+files+' file'+(files===1?'':'s');
    if(data.timed_out||data.truncated){
      // Caller may prepend notes via showStatus; keep summary in data attribute.
      tb.setAttribute('data-fm-summary', summary);
    } else {
      showStatus(summary, false);
    }
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
          var tbFail=document.getElementById('fm-rows');
          if(tbFail) setRowsMessage(tbFail, 'Listing unavailable.');
          return;
        }
        var notes=[];
        if(data.timed_out) notes.push('Listing stopped after a time limit so the panel stays responsive.');
        if(data.truncated) notes.push('Showing a bounded set of entries. Open a narrower path for restore-sized directories.');
        renderRows(data);
        if(notes.length){
          var tb=document.getElementById('fm-rows');
          var summary=tb?tb.getAttribute('data-fm-summary'):'';
          showStatus((summary?summary+'. ':'')+notes.join(' '), false);
        }
      })
      .catch(function(err){
        showStatus(err&&err.message?err.message:'Could not list this directory.', true);
        var tb=document.getElementById('fm-rows');
        if(tb) setRowsMessage(tb, 'Listing failed.');
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
      if(i>=files.length){ goTo(pageHref(path,'notice='+encodeURIComponent('Upload finished'))); return; }
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
        goTo(pageHref(path,'edit='+encodeURIComponent(childPath(names[0]))));
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
