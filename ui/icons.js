'use strict';
window.QyfluxIcons=(()=>{
 const cache=new Map(),queue=[];let active=0;
 function pump(){while(active<3&&queue.length){const job=queue.shift();active++;Promise.resolve().then(job.run).then(job.resolve,job.reject).finally(()=>{active--;pump();});}}
 function enqueue(run){return new Promise((resolve,reject)=>{queue.push({run,resolve,reject});pump();});}
 function source(entry,refresh){
  if(!refresh&&cache.has(entry.path))return cache.get(entry.path);
  const request=enqueue(()=>window.__TAURI__.core.invoke('get_entry_icon',{entryId:entry.id,refresh}).then(path=>window.__TAURI__.core.convertFileSrc(path)));
  cache.set(entry.path,request);request.catch(()=>{if(cache.get(entry.path)===request)cache.delete(entry.path);});return request;
 }
 function load(entry,container){
  if(!window.__TAURI__)return;
  const token=Symbol();container._iconToken=token;container.dataset.iconState='loading';
  (async()=>{
   let error;
   for(let attempt=0;attempt<3;attempt++){
    try{
     const src=await source(entry,attempt>0),img=new Image();img.alt='';img.draggable=false;img.className='system-icon';
     await new Promise((resolve,reject)=>{img.onload=resolve;img.onerror=()=>reject(new Error('图标文件加载失败'));img.src=src;});
     if(container._iconToken!==token)return;
     container.replaceChildren(img);container.classList.add('has-system-icon');container.classList.remove('icon-unavailable');container.dataset.iconState='ready';return;
    }catch(e){error=e;if(attempt<2)await new Promise(r=>setTimeout(r,300*(attempt+1)));}
   }
   if(container._iconToken!==token)return;
   container.dataset.iconState='error';container.classList.add('icon-unavailable');container.title=`图标不可用：${String(error)}；可点 ↻ 重新绑定文件`;
  })();
 }
 return{load};
})();