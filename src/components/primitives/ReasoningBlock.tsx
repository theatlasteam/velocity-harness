import React,{useState} from 'react';
import {ChevronDown} from '@geist-ui/icons';

/**
 * Model reasoning, visible but collapsed by default.
 * Streams in while the model thinks; expands into a scrollable body.
 */
export default function ReasoningBlock({text,complete,streaming}:{text:string;complete?:boolean;streaming?:boolean}){
  const [open,setOpen]=useState(false);
  const live=!!streaming&&!complete;
  return <div className="native-reasoning">
    <button type="button" className="reasoning-summary" aria-expanded={open} onClick={()=>setOpen(v=>!v)}>
      <span className={'reasoning-label'+(live?' live':'')}>{live?'Думает…':'Рассуждение'}</span>
      {!live&&<span className="reasoning-meta">{Math.max(1,Math.round((text||'').length/4))} зн.</span>}
      <ChevronDown size={14} className="reasoning-chevron"/>
    </button>
    <div className={'reasoning-body'+(open?' open':'')}>
      <div className="reasoning-overflow"><p>{text}</p></div>
    </div>
  </div>;
}
