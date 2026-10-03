// Run against npm run dev. These are UI tests with a mocked Tauri bridge and an
// intercepted model stream; Rust integration tests separately exercise the
// agent/tool protocol against the real engine.
const {chromium}=require('playwright');const assert=require('node:assert/strict');const cp=require('node:child_process');
const toolCallSSE='data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call1","function":{"name":"write_file","arguments":"{\\"path\\":\\"test.txt\\",\\"content\\":\\"hello\\"}"}}]},"finish_reason":"tool_calls"}]}\n\ndata: [DONE]\n\n';
const textSSE='data: {"choices":[{"delta":{"content":"Понял, действие отклонено."}},{"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n';
(async()=>{const browser=await chromium.launch({executablePath:process.env.CHROMIUM_BIN||cp.execSync('command -v chromium').toString().trim(),headless:true,args:['--no-sandbox']});const errors=[];for(const android of [false,true]){let modelCalls=0;const store={events:[],pending:null};const p=await browser.newPage({viewport:{width:android?390:1440,height:android?844:900},reducedMotion:'reduce'});p.on('pageerror',e=>errors.push(e.message));await p.route('**/chat/completions',r=>{modelCalls++;return r.fulfill({contentType:'text/event-stream',body:modelCalls===1?toolCallSSE:textSSE})});await p.addInitScript(() => {
 window.isTauri=true; window.__calls=[];window.__store={events:[],pending:null};
 window.__TAURI_INTERNALS__={invoke:async (cmd,args) => {
  window.__calls.push({cmd,args});
  if(cmd==='platform') return {android};
  if(cmd==='discover') return [{name:'Velocity Harness',url:'http://192.168.1.5:8787'}];
  if(cmd==='host_start') return {token:'mock-token',port:8787};
  if(cmd==='rpc') {
   const store=window.__store;
   if(args.method==='configure') return {ok:true};
   if(args.method==='metadata') return {provider:{name:'Linux provider',model:'test-model'},project:'/home/test/project'};
   if(args.method==='projects') return [];
   if(args.method==='create_chat') return {id:'test-id'};
   if(args.method==='sessions') return [{id:'test-id',title:'Новый чат',project_id:null,permission:'ask',events:store.events,pending:store.pending}];
   if(args.method==='agent_message'){const ev={id:'u1',type:'user',text:args.body.display_text};store.events.push(ev);return{session:'test-id',history:[{role:'user',content:args.body.text}],title:'Создай файл',user_event:ev,provider:{protocol:'openai',base_url:'https://mock.invalid/v1',model:'m'},permission:'ask',enabled:true,root:'/tmp'};}
   if(args.method==='provider_key') return 'k';
   if(args.method==='agent_append') return {ok:true};
   if(args.method==='agent_event'){const e=args.body.event;const i=store.events.findIndex(x=>x.id===e.id);if(i<0)store.events.push(e);else store.events[i]=e;return{ok:true};}
   if(args.method==='agent_pending'){store.pending=args.body.grant?{grant:args.body.grant,calls:args.body.calls}:null;return{ok:true};}
   if(args.method==='agent_preview') return {preview:{path:'test.txt',before:'',after:'hello',existed:false}};
   if(args.method==='agent_tool') return {result:{error:'Действие отклонено'},preview:null};
  }
  return null;
 }};
});await p.goto('http://localhost:5173');await p.waitForTimeout(600);if(android){await p.getByRole('button',{name:'Открыть меню'}).click();await p.getByRole('button',{name:'Подключение',exact:true}).click();await p.getByRole('button',{name:'Найти в локальной сети'}).click();await p.getByRole('button',{name:/Velocity Harness http:/}).click();await p.getByLabel('Токен из настроек Linux').fill('mock-token');await p.getByRole('button',{name:'Подключиться к Linux',exact:true}).click();await p.getByText('На Linux удалённо').waitFor()}else{await p.getByRole('button',{name:'Настройки',exact:true}).click();await p.getByLabel('Модель',{exact:true}).fill('test-model');await p.getByRole('button',{name:'Сохранить провайдер и проект'}).click()}
await p.getByLabel('Сообщение для Velocity').fill('Создай файл');await p.getByRole('button',{name:'Отправить сообщение'}).click();await p.getByText('Нужно твоё подтверждение').waitFor();await p.waitForTimeout(400);await p.screenshot({path:`/data/approval-${android?'android':'linux'}.png`});await p.getByRole('button',{name:'Отклонить',exact:true}).click();await p.getByText(/Действие отклонено/).waitFor();assert.equal(await p.getByRole('button',{name:'Отклонить',exact:true}).count(),0);const calls=await p.evaluate(()=>window.__calls);const tool=calls.find(c=>c.args?.method==='agent_tool');assert.ok(tool);assert.equal(tool.args.body.allow,false);assert.equal(await p.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);console.log(android?'Android remote UI passed':'Linux local UI passed');await p.close()}assert.deepEqual(errors,[]);await browser.close()})().catch(e=>{console.error(e);process.exit(1)});
