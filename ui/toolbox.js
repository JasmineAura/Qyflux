'use strict';
const $=id=>document.getElementById(id), api=window.__TAURI__;
const invoke=(cmd,args={})=>api?api.core.invoke(cmd,args):Promise.reject('请在桌面应用中使用');
let settings,activeTab=localStorage.getItem('toolbox-tab')||'default',appPage=0,tabPage=0,pageCount=1,toastTimer,lastWheel=0;
let hitSectors=[],selectedSector=null,lastPointer=null;
const safe=fn=>async(...args)=>{try{await fn(...args);}catch(e){toast(e);}};
function toast(message){$('toast').textContent=String(message);$('toast').classList.add('show');clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('toast').classList.remove('show'),3000);}
function applyAppearance(s){
 // The wheel keeps an opaque black surface independently of the main-window theme.
 document.body.classList.remove('has-wallpaper');document.body.dataset.material='clear';
 document.documentElement.style.setProperty('--window-wallpaper','none');
 document.documentElement.style.setProperty('--accent',s.accent);
 document.documentElement.style.setProperty('--accent-rgb',[0,2,4].map(i=>parseInt(s.accent.slice(i+1,i+3),16)).join(','));
 document.documentElement.style.setProperty('--surface-alpha',1);
 document.body.classList.toggle('no-motion',!s.motion);document.body.classList.toggle('no-glow',!s.glow);
}
function ringSpec(){
 const max=Math.min(innerWidth,innerHeight)/2-38,rings=[];
 for(let i=0;i<3;i++){const radius=146+i*90;if(radius>max&&rings.length)break;rings.push({radius:Math.min(radius,max),capacity:8+i*4});}
 return rings;
}
function position(node,radius,angle){node.style.setProperty('--x',`${Math.cos(angle)*radius}px`);node.style.setProperty('--y',`${Math.sin(angle)*radius}px`);}
function sectorPath(cx,cy,inner,outer,start,end){
 const point=(r,a)=>`${cx+Math.cos(a)*r},${cy+Math.sin(a)*r}`;
 if(end-start>=Math.PI*2-.001){const mid=start+Math.PI;return `M${point(outer,start)} A${outer},${outer} 0 0 1 ${point(outer,mid)} A${outer},${outer} 0 0 1 ${point(outer,end)} L${point(inner,end)} A${inner},${inner} 0 0 0 ${point(inner,mid)} A${inner},${inner} 0 0 0 ${point(inner,start)} Z`;}
 const large=end-start>Math.PI?1:0;
 return `M${point(inner,start)} L${point(outer,start)} A${outer},${outer} 0 ${large} 1 ${point(outer,end)} L${point(inner,end)} A${inner},${inner} 0 ${large} 0 ${point(inner,start)} Z`;
}
function chooseTab(id){activeTab=id;appPage=0;tabPage=Math.floor(Math.max(0,settings.tabs.findIndex(t=>t.id===id))/8);render();}
function activateSector(next,focus=false){
 if(selectedSector===next)return;
 if(selectedSector){selectedSector.sector.classList.remove('active');selectedSector.node.classList.remove('sector-active');}
 selectedSector=next;
 if(next){next.sector.classList.add('active');next.node.classList.add('sector-active');$('orbit-title').textContent=next.entry.name;$('orbit-title').title=next.entry.name;$('orbit-count').textContent='ENTER ↵';if(focus)next.node.focus({preventScroll:true});}
 else{$('orbit-title').textContent=settings.tabs.find(t=>t.id===activeTab)?.name||'常用';$('orbit-title').removeAttribute('title');$('orbit-count').textContent=`${settings.shortcuts.filter(e=>(e.tabId||'default')===activeTab).length} ITEMS`;}
}
function pointerSector(ev){
 lastPointer={clientX:ev.clientX,clientY:ev.clientY};
 const box=$('orbit-sectors').getBoundingClientRect(),x=ev.clientX-box.left-box.width/2,y=ev.clientY-box.top-box.height/2,radius=Math.hypot(x,y),angle=Math.atan2(y,x),tau=Math.PI*2;
 const next=hitSectors.find(s=>radius>=s.inner&&radius<=s.outer&&((angle-s.start)%tau+tau)%tau<=s.span+.00001)||null;
 activateSector(next,Boolean(next));
 if(!next&&document.activeElement?.classList.contains('orbit-app'))document.activeElement.blur();
}
function render(){
 activateSector(null);hitSectors=[];
 if(!settings.tabs.some(t=>t.id===activeTab))activeTab=settings.tabs[0]?.id||'default';
 localStorage.setItem('toolbox-tab',activeTab);
 const rings=ringSpec(),capacity=rings.reduce((sum,r)=>sum+r.capacity,0),entries=settings.shortcuts.filter(e=>(e.tabId||'default')===activeTab);
 pageCount=Math.max(1,Math.ceil(entries.length/capacity));appPage=Math.max(0,Math.min(appPage,pageCount-1));
 const visible=entries.slice(appPage*capacity,(appPage+1)*capacity);
 $('orbit-tracks').replaceChildren();
 const boundaries=[112,...rings.map((r,i)=>i<rings.length-1?(r.radius+rings[i+1].radius)/2:r.radius+44)];
 for(const radius of [82,...boundaries]){const line=document.createElement('div');line.className='orbit-track';line.style.width=`${radius*2}px`;line.style.height=`${radius*2}px`;$('orbit-tracks').append(line);}
 $('orbit-tabs').replaceChildren();tabPage=Math.min(tabPage,Math.max(0,Math.ceil(settings.tabs.length/8)-1));
 const tabs=settings.tabs.slice(tabPage*8,tabPage*8+8);
 tabs.forEach((tab,i)=>{const node=document.createElement('button');node.className='orbit-tab';node.textContent=tab.name;node.title=tab.name;node.dataset.id=tab.id;node.role='tab';node.setAttribute('aria-selected',String(tab.id===activeTab));position(node,82,-Math.PI/2+i*Math.PI*2/tabs.length);node.addEventListener('click',()=>chooseTab(tab.id));$('orbit-tabs').append(node);});
 $('orbit-more-tabs').hidden=settings.tabs.length<=8;
 $('orbit-apps').replaceChildren();$('orbit-sectors').replaceChildren();
 const width=$('toolbox').clientWidth,height=$('toolbox').clientHeight;$('orbit-sectors').setAttribute('viewBox',`0 0 ${width} ${height}`);
 let offset=0;
 rings.forEach((ring,layer)=>{
  const items=visible.slice(offset,offset+ring.capacity);offset+=items.length;
  items.forEach((entry,i)=>{
   const node=document.createElement('button');node.className='orbit-app';node.title=`${entry.name}\n${entry.path}`;node.setAttribute('aria-label',entry.name);node.dataset.layer=layer;node.dataset.id=entry.id;node.style.setProperty('--i',i+layer*3);
   const angle=-Math.PI/2+i*Math.PI*2/items.length+(layer%2?Math.PI/items.length:0);
   position(node,ring.radius,angle);
   const sector=document.createElementNS('http://www.w3.org/2000/svg','path');sector.classList.add('orbit-sector');sector.dataset.id=entry.id;sector.dataset.layer=layer;
   sector.setAttribute('d',sectorPath(width/2,height/2,boundaries[layer],boundaries[layer+1],angle-Math.PI/items.length,angle+Math.PI/items.length));
   const icon=document.createElement('span');icon.className='orbit-icon';icon.textContent=entry.kind==='folder'?'▱':'◇';window.QyfluxIcons.load(entry,icon);
   const name=document.createElement('span');name.className='orbit-app-name';name.textContent=entry.name;node.append(icon,name);
   const launch=safe(async()=>{await invoke('launch_entry',{entryId:entry.id});await api.window.getCurrentWindow().hide();});
   node.addEventListener('click',launch);sector.addEventListener('click',launch);
   const hit={sector,node,entry,inner:boundaries[layer],outer:boundaries[layer+1],start:angle-Math.PI/items.length,span:Math.PI*2/items.length};hitSectors.push(hit);
   node.addEventListener('focus',()=>activateSector(hit));
   $('orbit-sectors').append(sector);
   $('orbit-apps').append(node);
  });
 });
 $('orbit-title').textContent=settings.tabs.find(t=>t.id===activeTab)?.name||'常用';$('orbit-count').textContent=`${entries.length} ITEMS`;
 $('orbit-pages').hidden=pageCount===1;$('orbit-page').textContent=`${String(appPage+1).padStart(2,'0')} / ${String(pageCount).padStart(2,'0')}`;
 $('orbit-empty').hidden=entries.length>0;
 if(lastPointer)requestAnimationFrame(()=>pointerSector(lastPointer));
}
function turnPage(delta){if(pageCount===1)return;appPage=(appPage+delta+pageCount)%pageCount;render();}
async function init(){
 settings=api?await invoke('get_settings'):{accent:'#a69aff',opacity:.84,material:'clear',motion:true,glow:true,tabs:[{id:'default',name:'常用'}],shortcuts:[]};
 applyAppearance(settings);render();
 if(api){
  await api.event.listen('settings-changed',({payload})=>{const dataChanged=JSON.stringify([settings.tabs,settings.shortcuts])!==JSON.stringify([payload.tabs,payload.shortcuts]);const lookChanged=['accent','opacity','material','background','motion','glow'].some(k=>settings[k]!==payload[k]);settings=payload;if(lookChanged)applyAppearance(settings);if(dataChanged)render();});
  await api.event.listen('select-tab',({payload})=>{if(settings.tabs.some(t=>t.id===payload))chooseTab(payload);});
  await api.event.listen('focus-search',()=>{$('toolbox').classList.remove('orbit-arrive');void $('toolbox').offsetWidth;$('toolbox').classList.add('orbit-arrive');});
  settings=await invoke('get_settings');applyAppearance(settings);render();
 }
 $('orbit-close').addEventListener('click',safe(()=>api.window.getCurrentWindow().hide()));
 document.addEventListener('pointermove',pointerSector);
 document.addEventListener('pointerover',pointerSector);
 document.documentElement.addEventListener('pointerleave',()=>activateSector(null));
 $('orbit-prev').addEventListener('click',()=>turnPage(-1));$('orbit-next').addEventListener('click',()=>turnPage(1));
 $('orbit-more-tabs').addEventListener('click',()=>{tabPage=(tabPage+1)%Math.ceil(settings.tabs.length/8);render();});
 document.addEventListener('wheel',ev=>{ev.preventDefault();if(settings.tabs.length<2||Date.now()-lastWheel<280||Math.abs(ev.deltaY)<8)return;lastWheel=Date.now();const index=settings.tabs.findIndex(t=>t.id===activeTab),delta=ev.deltaY>0?1:-1;chooseTab(settings.tabs[(index+delta+settings.tabs.length)%settings.tabs.length].id);},{passive:false});
 document.addEventListener('keydown',safe(async ev=>{
  if(ev.key==='Escape'){await api.window.getCurrentWindow().hide();return;}
  if(ev.ctrlKey&&(ev.key==='ArrowLeft'||ev.key==='ArrowRight')){ev.preventDefault();const at=settings.tabs.findIndex(t=>t.id===activeTab),delta=ev.key==='ArrowRight'?1:-1;chooseTab(settings.tabs[(at+delta+settings.tabs.length)%settings.tabs.length].id);return;}
  if(ev.key==='PageDown'||ev.key==='PageUp'){ev.preventDefault();turnPage(ev.key==='PageDown'?1:-1);return;}
  const cards=[...document.querySelectorAll('.orbit-app')],at=cards.indexOf(document.activeElement),delta={ArrowRight:1,ArrowDown:1,ArrowLeft:-1,ArrowUp:-1}[ev.key];
  if(delta&&cards.length){ev.preventDefault();cards[at<0?0:(at+delta+cards.length)%cards.length].focus();}
 }));
 let previousCapacity=ringSpec().reduce((sum,r)=>sum+r.capacity,0);
 window.addEventListener('resize',()=>{const capacity=ringSpec().reduce((sum,r)=>sum+r.capacity,0);if(capacity!==previousCapacity){previousCapacity=capacity;appPage=0;}render();});
}
init().catch(toast);
