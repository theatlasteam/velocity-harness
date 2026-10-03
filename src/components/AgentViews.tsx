import React,{useState,useMemo} from 'react';
import {structuredPatch} from 'diff';
import {Terminal,FileText,Shield} from '@geist-ui/icons';
import CodeBlock,{type DiffRow} from './primitives/CodeBlock';
import ToolChips from './primitives/ToolChips';
import ApprovalCard from './primitives/ApprovalCard';
import RecommendationCard from './primitives/RecommendationCard';
import type {AgentEvent,Call,Pending,Preview} from '../types';
export function FileDiff({preview}:{preview:Preview}){
 const rows=useMemo(()=>{const patch=structuredPatch(preview.path,preview.path,preview.before,preview.after,'','',{context:3});const rows:DiffRow[]=[];for(const h of patch.hunks){let old=h.oldStart,cur=h.newStart;for(const line of h.lines){if(line.startsWith('\\'))continue;const type=line[0]==='+'?'add':line[0]==='-'?'del':'ctx';rows.push({old:type==='add'?null:old++,cur:type==='del'?null:cur++,type,pieces:[{text:line.slice(1)}]});if(rows.length>=1200)break;}if(rows.length>=1200)break;}return rows},[preview]);
 return <div className="file-review"><CodeBlock variant="Diff" filename={preview.path} code={preview.after} diff={rows} labels={{copy:'Копировать',copied:'Скопировано'}}/>{rows.length>=1200&&<p className="muted">Показаны первые 1200 строк diff. Большую правку лучше разделить.</p>}</div>
}
function args(value?:string){try{return JSON.parse(value||'{}')}catch{return {}}}
export function ToolDetails({event}:{event:AgentEvent}){
 const [tab,setTab]=useState('output');const a=args(event.arguments);const preview=event.preview||event.result?.preview;
 if(preview&&['edit_file','write_file'].includes(event.name||''))return <><FileDiff preview={preview}/><p className={event.result?.error?'risk-note':'muted'} role="status">{event.result?.error|| (event.status==='running'?'Применяю правку…':'Правка применена')}</p></>;
 return <div className="native-tool-panel"><div className="native-tool-tabs" role="tablist" aria-label="Детали инструмента">{['command','output'].map(v=><button key={v} role="tab" aria-selected={tab===v} onClick={()=>setTab(v)}>{v==='command'?'Command':'Output'}</button>)}</div><pre>{tab==='command'?(event.name==='shell'?a.command:JSON.stringify({tool:event.name,...a},null,2)):`status: ${event.status||'exited'}\nexit_code: ${event.result?.code??(event.result?.error?'error':'0')}\n\nstdout:\n${typeof event.result?.stdout==='string'?event.result.stdout:JSON.stringify(event.result,null,2)}\n\nstderr:\n${event.result?.stderr||''}`}</pre></div>
}
export function ToolCall({event,onRecommend,disabled}:{event:AgentEvent;onRecommend:(text:string)=>void;disabled:boolean}){
 const a=args(event.arguments);
 if(event.name==='recommend_options'&&Array.isArray(event.result?.options)&&event.result.options.length){const options=event.result.options.map((o:any,i:number)=>({key:String(i),body:<>{String(o.body||o.short)}</>,short:String(o.short||o.body),signal:0,tone:'var(--ink-3)',label:'Вариант',cta:'Продолжить',ctaVariant:'primary' as const}));return <RecommendationCard options={options} labels={{title:event.result.title||'Как продолжим?',alternatives:'Альтернативы',otherOptions:'Другие варианты',accepted:'Выбрано'}} onAccept={o=>onRecommend('Выбираю вариант: '+o.short)} disabled={disabled}/>}
 return <div className="native-tool-call"><ToolChips live diffs={[]} labels={{header:event.status==='running'?'Инструмент работает':'Действие агента',more:''}} steps={[{icon:event.name==='shell'?'run':event.name==='read_file'?'read':'write',label:event.name||'tool',chip:a.path||a.command||a.query||a.pattern||'Детали',mono:true,detailMono:true,detail:[],detailContent:<ToolDetails event={event}/>}]} /></div>
}
function ActionReview({call}:{call:Call}){const a=args(call.function.arguments);const name=call.function.name;
 return <div className="action-review"><div className="action-title">{name==='shell'?<Terminal size={16}/>:<FileText size={16}/>}<strong>{a.path||name}</strong><span>{name==='write_file'?call.preview?.existed?'Перезапись':'Создание':name==='edit_file'?'Изменение':name==='shell'?'Команда':'Действие'}</span></div>{call.preview?<FileDiff preview={call.preview}/>:name==='shell'?<><pre className="command-review">{a.command}</pre><p className="risk-note">Команда запускается от твоего имени. Shell не изолирован и может работать вне проекта.</p></>:<pre className="command-review">{JSON.stringify(a,null,2)}</pre>}</div>
}
export function ConfirmActions({pending,onDecision,busy,readOnly,remote}:{pending:Pending;onDecision:(allow:boolean)=>void;busy:boolean;readOnly:boolean;remote:boolean}){
 const shellOnly=pending.calls.length===1&&pending.calls[0].function.name==='shell';
 if(shellOnly&&!readOnly){
  const cmd=args(pending.calls[0].function.arguments).command||'';
  return <RecommendationCard disabled={busy} labels={{title:'Запустить команду?',alternatives:'Варианты',otherOptions:'Другие варианты',accepted:'Принято'}} options={[{key:'run',body:<><pre className="command-review">{String(cmd)}</pre><p className="risk-note">Команда запускается от твоего имени. Shell не изолирован и может работать вне проекта.</p></>,short:'Выполнить: '+String(cmd).slice(0,80),signal:2,tone:'var(--orange)',label:'Shell не изолирован',cta:'Выполнить',ctaVariant:'accent'},{key:'deny',body:<>Не выполнять эту команду. Агент продолжит без результата команды.</>,short:'Не выполнять',signal:0,tone:'var(--ink-3)',label:'Безопасно',cta:'Отклонить',ctaVariant:'secondary'}]} onAccept={o=>onDecision(o.key!=='deny')}/>;
 }
 return <ApprovalCard title="Подтвердить действие?" description={remote?'Выполнится на Linux-компьютере.':'Выполнится на этом компьютере.'} review={<>{pending.calls.map(c=><ActionReview key={c.id} call={c}/>)}</>} onDecision={onDecision} disabled={busy} readOnly={readOnly}/>}
