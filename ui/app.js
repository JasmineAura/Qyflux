'use strict';
const $ = id => document.getElementById(id);
const api = window.__TAURI__;
const zoneId = new URLSearchParams(location.search).get('zone');
let settings, toastTimer, boundsTimer, searchIndex = 0, appearanceDirty = false, switching = false;
let activeConfigTab='default', editingConfigTab=false, configEntry=null;
const invoke = async (command, args = {}) => {
  if (!api) throw new Error('请在 Qyflux 桌面应用中使用此功能');
  return api.core.invoke(command, args);
};
function toast(message) { $('toast').textContent = String(message); $('toast').classList.add('show'); clearTimeout(toastTimer); toastTimer = setTimeout(() => $('toast').classList.remove('show'), 3800); }
function safe(action) { return async (...args) => { try { return await action(...args); } catch (e) { toast(e); } }; }
function bind(id, action) { $(id).addEventListener('click', safe(action)); }
const icons = {
  app: '<path d="M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z"/>',
  folder: '<path d="M3 7V5h6l2 3h10v12H3z"/><path d="M3 8h8"/>',
  file: '<path d="M6 3h8l4 4v14H6zM14 3v5h4M9 12h6M9 16h6"/>',
};
function appearance(s) {
  const hasWallpaper = !zoneId && Boolean(s.background);
  document.body.classList.toggle('has-wallpaper', hasWallpaper);
  document.body.dataset.material = s.material;
  document.documentElement.style.setProperty('--accent', s.accent);
  const c = s.accent.slice(1); const rgb = [0, 2, 4].map(i => parseInt(c.slice(i, i + 2), 16)).join(',');
  document.documentElement.style.setProperty('--accent-rgb', rgb);
  document.documentElement.style.setProperty('--opacity', s.opacity);
  document.documentElement.style.setProperty('--surface-alpha', hasWallpaper ? 1 : s.material === 'clear' ? s.opacity : s.opacity * (s.material === 'blur' ? .27 : .43));
  document.body.classList.toggle('no-glow', !s.glow);
  document.body.classList.toggle('no-motion', !s.motion);
  const bg = !zoneId && s.background && api ? api.core.convertFileSrc(s.background) : '';
  document.documentElement.style.setProperty('--window-wallpaper', bg ? `url(${JSON.stringify(bg)})` : 'none');
  $('wallpaper').style.backgroundImage = bg ? `url(${JSON.stringify(bg)})` : '';
  // Keep wallpaper on its translucent layer so it never masks the native glass.
  $('app').style.backgroundImage = '';
  $('app').style.backgroundSize = 'cover';
  $('app').style.backgroundPosition = 'center';
}
function zoneAppearance() {
  if (!zoneId) return;
  const zone = settings.zones.find(z => z.id === zoneId);
  if (!zone) return;
  const color = zone.color || '#171a25', opacity = zone.opacity ?? .22;
  const rgb = [0,2,4].map(i => parseInt(color.slice(i+1,i+3),16)).join(',');
  document.documentElement.style.setProperty('--zone-color-rgb',rgb);
  document.documentElement.style.setProperty('--zone-opacity',opacity);
  $('zone-color').value=color;$('zone-opacity').value=Math.round(opacity*100);$('zone-opacity-value').textContent=`${Math.round(opacity*100)}%`;
}
function fillAppearance(s) {
  const wallpaperMode = Boolean(s.background);
  $('opacity').disabled = wallpaperMode;
  document.querySelectorAll('[data-effect]').forEach(b => { b.disabled = wallpaperMode; });
  $('wallpaper-note').hidden = !wallpaperMode;
  $('accent').value = s.accent; $('opacity').value = Math.round(s.opacity * 100); $('opacity-value').textContent = `${Math.round(s.opacity * 100)}%`;
  $('glow').checked = s.glow; $('motion').checked = s.motion;
  document.querySelectorAll('[data-color]').forEach(b => b.classList.toggle('selected', b.dataset.color.toLowerCase() === s.accent.toLowerCase()));
  document.querySelectorAll('[data-effect]').forEach(b => b.classList.toggle('selected', b.dataset.effect === s.material));
}
function entryCard(e, index, inZone) {
  const card = document.createElement('div'); card.className = 'entry'; card.style.setProperty('--i', Math.min(index, 12));
  const btn = document.createElement('button'); btn.className = 'entry-launch'; btn.title = e.path;
  const icon = document.createElement('span'); icon.className = 'entry-icon'; icon.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round">${icons[e.kind] || icons.file}</svg>`;
  window.QyfluxIcons.load(e, icon);
  const name = document.createElement('span'); name.className = 'entry-name'; name.textContent = e.name;
  btn.append(icon, name);
  const launch = safe(async () => { await invoke('launch_entry', { entryId: e.id }); if (!inZone) toast(`已打开 ${e.name}`); });
  btn.addEventListener(inZone ? 'dblclick' : 'click', launch);
  btn.addEventListener('keydown', ev => { if (inZone && ev.key === 'Enter') { ev.preventDefault(); launch(); } });
  const remove = document.createElement('button'); remove.className = 'entry-remove'; remove.textContent = '×'; remove.title = '移除引用，保留原文件'; remove.setAttribute('aria-label', `移除 ${e.name}`);
  const erase = safe(async () => { settings = await invoke('remove_entry', { zoneId: inZone ? zoneId : null, entryId: e.id }); renderEntries(); });
  remove.addEventListener('click', erase);
  card.addEventListener('contextmenu', ev => { ev.preventDefault(); if(inZone)erase();else configEntryMenu(ev,e); });
  const rebind=document.createElement('button');rebind.className='entry-rebind';rebind.textContent='↻';rebind.title='重新绑定文件，可修复失效的快捷方式';rebind.setAttribute('aria-label',`重新绑定 ${e.name}`);
  rebind.addEventListener('click',safe(async()=>{settings=await invoke('rebind_entry',{entryId:e.id});renderEntries();}));
  card.append(btn, remove, rebind); return card;
}
function renderEntries() {
  if (!settings) return;
  const inZone = Boolean(zoneId);
  const zone = settings.zones.find(z => z.id === zoneId);
  if (inZone) $('zone-title').textContent = zone?.name || '桌面区域';
  const all = inZone ? (zone?.entries || []) : settings.shortcuts.filter(e=>(e.tabId||'default')===activeConfigTab);
  const query = inZone ? '' : $('search').value.trim().toLocaleLowerCase();
  const entries = all.filter(e => `${e.name} ${e.path}`.toLocaleLowerCase().includes(query));
  const container = inZone ? $('zone-entries') : $('shortcuts'); container.replaceChildren();
  entries.forEach((e, i) => container.append(entryCard(e, i, inZone)));
  if (inZone) $('zone-empty').hidden = all.length > 0;
  else {
    $('entry-count').textContent = `${entries.length} ITEMS`;
    if (!entries.length) { const msg = document.createElement('div'); msg.className = 'empty'; msg.textContent = query ? '没有找到，试试其他关键词。' : '从添加一个快捷方式开始。'; container.append(msg); }
  }
}
function renderConfigTabs(){
  if(zoneId)return;
  if(!settings.tabs.some(t=>t.id===activeConfigTab))activeConfigTab='default';
  $('config-tabs').replaceChildren();
  for(const tab of settings.tabs){const btn=document.createElement('button');btn.textContent=tab.name;btn.className='toolbox-tab';btn.role='tab';btn.dataset.id=tab.id;btn.setAttribute('aria-selected',String(tab.id===activeConfigTab));btn.addEventListener('click',()=>{activeConfigTab=tab.id;$('search').value='';renderConfigTabs();renderEntries();});$('config-tabs').append(btn);}
}
function configTabDialog(edit){editingConfigTab=edit;$('config-dialog-title').textContent=edit?'编辑标签':'新建标签';$('config-tab-name').value=edit?settings.tabs.find(t=>t.id===activeConfigTab).name:'';$('config-delete-tab').hidden=!edit||activeConfigTab==='default';$('config-tab-dialog').showModal();$('config-tab-name').focus();}
function configEntryMenu(ev,entry){
  configEntry=entry;$('config-move-tabs').replaceChildren();
  for(const tab of settings.tabs.filter(t=>t.id!==entry.tabId)){const btn=document.createElement('button');btn.textContent=tab.name;btn.addEventListener('click',safe(async()=>{$('config-entry-menu').hidden=true;await invoke('move_entry',{entryId:entry.id,tabId:tab.id});}));$('config-move-tabs').append(btn);}
  const menu=$('config-entry-menu');menu.hidden=false;const box=menu.getBoundingClientRect();menu.style.left=`${Math.max(6,Math.min(ev.clientX,innerWidth-box.width-8))}px`;menu.style.top=`${Math.max(6,Math.min(ev.clientY,innerHeight-box.height-8))}px`;
}
function renderZones() {
  $('zones').replaceChildren();
  for (const z of settings.zones) {
    const card = document.createElement('div'); card.className = 'zone-card';
    const graphic = document.createElement('div'); graphic.className = 'zone-visual'; graphic.textContent = '▧';
    const name = document.createElement('strong'); name.textContent = z.name;
    const count = document.createElement('small'); count.textContent = `${z.entries.length} 个文件引用 · ${Math.round(z.width)} × ${Math.round(z.height)}`;
    const foot = document.createElement('div'); foot.className = 'zone-card-footer';
    const remove = document.createElement('button'); remove.className = 'text-button danger'; remove.textContent = '移除区域';
    remove.addEventListener('click', safe(async () => { settings = await invoke('remove_zone', { zoneId: z.id }); renderZones(); toast('区域已移除，原文件保持原位'); }));
    const open = document.createElement('button'); open.className = 'text-button'; open.textContent = '打开区域 ↗'; open.addEventListener('click', safe(() => invoke('show_zone', { zoneId: z.id })));
    foot.append(remove, open); card.append(graphic, name, count, foot); $('zones').append(card);
  }
  if (!settings.zones.length) { const empty = document.createElement('div'); empty.className = 'empty'; empty.textContent = '还没有桌面区域。创建一个，给灵感留个位置。'; $('zones').append(empty); }
}
function switchPage(name) {
  if (switching || $(`page-${name}`).classList.contains('active')) return;
  switching = true; $('app').classList.remove('switching'); void $('app').offsetWidth; $('app').classList.add('switching');
  document.querySelectorAll('.nav-item').forEach(b => b.classList.toggle('active', b.dataset.page === name));
  document.querySelectorAll('.page').forEach(p => p.classList.remove('active', 'entering'));
  const page = $(`page-${name}`); page.classList.add('active', 'entering');
  if (name === 'launcher') setTimeout(() => $('search').focus(), 100);
  setTimeout(() => { $('app').classList.remove('switching'); switching = false; }, 400);
}
function localAppearance() {
  return { ...settings, accent: $('accent').value, opacity: Number($('opacity').value) / 100, material: document.querySelector('[data-effect].selected')?.dataset.effect || settings.material, glow: $('glow').checked, motion: $('motion').checked };
}
function previewAppearance() {
  const s = localAppearance(); appearance(s); fillAppearance(s); appearanceDirty = true; $('save-state').textContent = '预览中 · 点击保存外观以应用窗口材质';
}
async function pick(folder = false) { settings = await invoke('pick_entries', { zoneId: zoneId || null, tabId:zoneId?null:activeConfigTab, folder }); renderEntries(); }
async function init() {
  if (zoneId) { document.body.classList.add('zone-mode'); $('app').hidden = true; $('zone-app').hidden = false; }
  if (api) {
    settings = await invoke('get_settings');
    const win = api.window.getCurrentWindow();
    await api.event.listen('settings-changed', ({ payload }) => {
      const entriesChanged = JSON.stringify(zoneId ? settings.zones.find(z => z.id === zoneId)?.entries : settings.shortcuts) !== JSON.stringify(zoneId ? payload.zones.find(z => z.id === zoneId)?.entries : payload.shortcuts);
      const appearanceChanged = ['accent','opacity','material','background','glow','motion'].some(k => settings[k] !== payload[k]);
      const zonesChanged = JSON.stringify(settings.zones) !== JSON.stringify(payload.zones);
      const tabsChanged = JSON.stringify(settings.tabs)!==JSON.stringify(payload.tabs);
      const oldZone = settings.zones.find(z=>z.id===zoneId), newZone = payload.zones.find(z=>z.id===zoneId);
      const zoneAppearanceChanged = oldZone?.color!==newZone?.color || oldZone?.opacity!==newZone?.opacity;
      settings = payload;
      if (zoneId && zoneAppearanceChanged) zoneAppearance();
      if (appearanceChanged && (!appearanceDirty || zoneId)) { appearance(settings); fillAppearance(settings); }
      if (tabsChanged) renderConfigTabs();
      if (entriesChanged||tabsChanged) renderEntries();
      if (zonesChanged && !zoneId) renderZones();
    });
    await api.event.listen('focus-search', () => { if (!zoneId) { switchPage('launcher'); $('search').focus(); $('search').select(); } });
    await win.onDragDropEvent(safe(async ({ payload }) => {
      if (payload.type === 'enter' || payload.type === 'over') $('drop-overlay').classList.add('show');
      else $('drop-overlay').classList.remove('show');
      if (payload.type === 'drop') { settings = await invoke('add_paths', { zoneId: zoneId || null, tabId:zoneId?null:activeConfigTab, paths: payload.paths }); renderEntries(); toast('已添加到你的空间'); }
    }));
    if (zoneId) {
      const saveBounds = () => { clearTimeout(boundsTimer); boundsTimer = setTimeout(safe(() => invoke('save_zone_bounds')), 500); };
      await win.onMoved(saveBounds); await win.onResized(saveBounds);
    } else {
      try { $('autostart').checked = await invoke('autostart_status'); } catch(e) { toast(e); }
      try { const error = await invoke('shortcut_error'); if (error) { $('shortcut-status').textContent = '快捷键注册失败，可能被占用；可通过托盘打开'; toast('Alt+Space 被其他程序占用，可通过托盘打开'); } } catch(e) { toast(e); }
    }
    bind('hide', () => win.hide()); bind('minimize', () => win.minimize()); bind('zone-hide', () => win.hide());
  } else {
    settings = { accent: '#a69aff', opacity: .94, material: 'acrylic', glow: true, motion: true, background: null, shortcuts: [{ id:'preview-1',name:'文件资源管理器',kind:'folder',path:'C:\\Windows\\explorer.exe' },{id:'preview-2',name:'记事本',kind:'app',path:'C:\\Windows\\System32\\notepad.exe'},{id:'preview-3',name:'计算器',kind:'app',path:'C:\\Windows\\System32\\calc.exe'}], zones: [] };
    $('save-state').textContent = '浏览器预览 · 原生功能请使用桌面应用';
  }
  // Re-read after subscribing so updates during WebView startup are never lost.
  if (api) settings = await invoke('get_settings');
  if(!settings.tabs)settings.tabs=[{id:'default',name:'常用'}];
  appearance(settings); fillAppearance(settings); zoneAppearance(); renderConfigTabs(); renderEntries(); renderZones();
  bind('zone-style', () => { $('zone-style-panel').hidden = !$('zone-style-panel').hidden; });
  const previewZone = () => {
    const color=$('zone-color').value;
    document.documentElement.style.setProperty('--zone-color-rgb',[0,2,4].map(i=>parseInt(color.slice(i+1,i+3),16)).join(','));
    document.documentElement.style.setProperty('--zone-opacity',Number($('zone-opacity').value)/100);
    $('zone-opacity-value').textContent=`${$('zone-opacity').value}%`;
  };
  $('zone-color').addEventListener('input',previewZone);$('zone-opacity').addEventListener('input',previewZone);
  bind('zone-style-save',async()=>{await invoke('save_zone_appearance',{zoneId,color:$('zone-color').value,opacity:Number($('zone-opacity').value)/100});$('zone-style-panel').hidden=true;toast('区域外观已保存');});
  document.querySelectorAll('[data-page]').forEach(b => b.addEventListener('click', () => switchPage(b.dataset.page)));
  $('search').addEventListener('input', () => { searchIndex = 0; renderEntries(); });
  $('search').addEventListener('keydown', ev => { const cards = [...$('shortcuts').querySelectorAll('.entry-launch')]; if (ev.key === 'Enter' && cards[0]) cards[0].click(); if (ev.key === 'ArrowDown' && cards.length) { ev.preventDefault(); cards[searchIndex % cards.length].focus(); searchIndex++; } });
  document.addEventListener('keydown', safe(async ev => { if(ev.key==='Escape'&&$('config-tab-dialog').open)return;if(ev.key==='Escape'&&!$('config-entry-menu').hidden){$('config-entry-menu').hidden=true;return;} if (ev.key === 'Escape') { if (!zoneId && $('search').value) { $('search').value = ''; renderEntries(); } else if (api) await api.window.getCurrentWindow().hide(); } }));
  bind('add-entry', () => pick()); bind('add-folder', () => pick(true)); bind('zone-add', () => pick());
  bind('open-toolbox', () => invoke('show_toolbox',{tabId:activeConfigTab}));
  bind('config-new-tab',()=>configTabDialog(false));bind('config-edit-tab',()=>configTabDialog(true));bind('config-cancel-tab',()=>$('config-tab-dialog').close());
  $('config-tab-form').addEventListener('submit',safe(async ev=>{ev.preventDefault();const before=new Set(settings.tabs.map(t=>t.id));const next=await invoke(editingConfigTab?'rename_tab':'create_tab',{tabId:activeConfigTab,name:$('config-tab-name').value});settings=next;if(!editingConfigTab)activeConfigTab=next.tabs.find(t=>!before.has(t.id)).id;$('config-tab-dialog').close();renderConfigTabs();renderEntries();}));
  bind('config-delete-tab',async()=>{await invoke('remove_tab',{tabId:activeConfigTab});$('config-tab-dialog').close();});
  bind('config-remove-entry',async()=>{$('config-entry-menu').hidden=true;await invoke('remove_entry',{zoneId:null,entryId:configEntry.id});});
  document.addEventListener('pointerdown',ev=>{if(!$('config-entry-menu').contains(ev.target))$('config-entry-menu').hidden=true;});
  bind('create-zone', async () => { const name = $('zone-name').value.trim(); if (!name) { $('zone-name').focus(); toast('先给区域起个名字'); return; } settings = await invoke('create_zone', { name }); $('zone-name').value = ''; renderZones(); toast('透明区域已创建，可以拖入文件'); });
  $('zone-name').addEventListener('keydown', ev => { if (ev.key === 'Enter') $('create-zone').click(); });
  document.querySelectorAll('[data-color]').forEach(b => b.addEventListener('click', () => { $('accent').value = b.dataset.color; previewAppearance(); }));
  document.querySelectorAll('[data-effect]').forEach(b => b.addEventListener('click', () => { document.querySelectorAll('[data-effect]').forEach(other => other.classList.toggle('selected', other === b)); previewAppearance(); }));
  for (const id of ['accent', 'opacity', 'glow', 'motion']) $(id).addEventListener('input', previewAppearance);
  bind('save-appearance', async () => {
    const s = localAppearance(); const button = $('save-appearance'); button.disabled = true;
    try { settings = await invoke('save_appearance', { accent: s.accent, opacity: s.opacity, effect: s.material, glow: s.glow, motion: s.motion }); appearanceDirty = false; $('save-state').textContent = '外观已保存'; toast('光影就位'); } finally { button.disabled = false; }
  });
  bind('pick-bg', async () => { settings = await invoke('pick_background'); const preview=appearanceDirty?localAppearance():settings; appearance(preview); fillAppearance(preview); });
  bind('clear-bg', async () => { settings = await invoke('clear_background'); const preview=appearanceDirty?localAppearance():settings; appearance(preview); fillAppearance(preview); });
  $('autostart').addEventListener('change', safe(async () => { const checked = $('autostart').checked; try { await invoke('set_autostart', { enabled: checked }); toast(checked ? '已开启开机自启动' : '已关闭开机自启动'); } catch(e) { $('autostart').checked = !checked; throw e; } }));
  bind('quit', () => invoke('exit_app'));
  if (!zoneId) { $('page-launcher').classList.add('entering'); $('search').focus(); }
}
init().catch(toast);
