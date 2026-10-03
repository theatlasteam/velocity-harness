// Run against npm run dev. These are UI tests with a mocked Tauri bridge;
// Rust integration tests separately exercise real HTTP provider/tool protocols.
const {chromium}=require('playwright');const assert=require('node:assert/strict');const cp=require('node:child_process');
(async()=>{const browser=await chromium.launch({executablePath:process.env.CHROMIUM_BIN||cp.execSync('command -v chromium').toString().trim(),headless:true,args:['--no-sandbox']});const errors=[];for(const android of [false,true]){const p=await browser.newPage({viewport:{width:android?390:1440,height:android?844:900},reducedMotion:'reduce'});p.on('pageerror',e=>errors.push(e.message));await p.addInitScript(({android}) => {
 window.isTauri=true; window.__calls=[];
 window.__TAURI_INTERNALS__={invoke:async (cmd,args) => {
  window.__calls.push({cmd,args});
  if(cmd==='platform') return {android};
  if(cmd==='discover') return [{name:'Velocity Harness',url:'http://192.168.1.5:8787'}];
  if(cmd==='host_start') return {token:'mock-token',port:8787};
  if(cmd==='rpc') {
   if(args.method==='configure') return {ok:true};
   if(args.method==='metadata') return {provider:{name:'Linux provider',model:'test-model'},project:'/home/test/project'};
   if(args.method==='sessions') return [];
   if(args.method==='turn') return {session:'test-id',events:[{type:'text',text:'Проверил проект. Предлагаю исправление.'}],pending:{grant:'one-use-grant',calls:[{function:{name:'write_file',arguments:JSON.stringify({path:'test.txt',content:'hello'})}}]}};
   if(args.method==='approve') return {session:'test-id',events:[{type:'text',text:args.body.allow?'Файл записан.':'Действие отклонено.'}],pending:null};
  }
  return null;
 }};
},{android});await p.goto('http://localhost:5173');await p.waitForTimeout(600);if(android){await p.getByRole('button',{name:'Открыть меню'}).click();await p.getByRole('button',{name:'Подключение',exact:true}).click();await p.getByRole('button',{name:'Найти в локальной сети'}).click();await p.getByRole('button',{name:/Velocity Harness http:/}).click();await p.getByLabel('Токен из настроек Linux').fill('mock-token');await p.getByRole('button',{name:'Подключиться к Linux',exact:true}).click();await p.getByText('На Linux удалённо').waitFor()}else{await p.getByRole('button',{name:'Настройки',exact:true}).click();await p.getByLabel('Модель',{exact:true}).fill('test-model');await p.getByRole('button',{name:'Сохранить провайдер и проект'}).click()}
await p.getByLabel('Сообщение для Velocity').fill('Создай файл');await p.getByRole('button',{name:'Отправить сообщение'}).click();await p.getByText('Нужно твоё подтверждение').waitFor();await p.waitForTimeout(400);await p.screenshot({path:`/data/approval-${android?'android':'linux'}.png`});await p.getByRole('button',{name:'Отклонить',exact:true}).click();await p.getByText('Действие отклонено.').waitFor();assert.equal(await p.getByRole('button',{name:'Отклонить',exact:true}).count(),0);const calls=await p.evaluate(()=>window.__calls);assert.equal(calls.find(c=>c.args?.method==='approve').args.body.allow,false);assert.equal(await p.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);console.log(android?'Android remote UI passed':'Linux local UI passed');await p.close()}assert.deepEqual(errors,[]);await browser.close()})().catch(e=>{console.error(e);process.exit(1)});
