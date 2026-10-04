import React, { useEffect, useState } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { readPlan } from '../lib/plans';

export default function PlanArtifact({ name, onClose, onAccept, onFeedback }: {
  name: string;
  onClose: () => void;
  onAccept: () => void;
  onFeedback: (text: string) => void;
}) {
  const [content, setContent] = useState('Loading plan…');
  const [fb, setFb] = useState('');
  const [fbOpen, setFbOpen] = useState(false);
  useEffect(() => {
    let live = true;
    readPlan(name).then(r => { if (live) setContent(r.content || '(empty plan)'); })
      .catch(e => { if (live) setContent(String(e)); });
    return () => { live = false; };
  }, [name]);
  return (
    <aside className="plan-artifact" aria-label="Plan artifact">
      <div className="plan-artifact-head">
        <div><strong>plan.md</strong><small>/tmp/vhr/{name}.md</small></div>
        <button className="icon-button" onClick={onClose} aria-label="Close plan">✕</button>
      </div>
      <div className="plan-artifact-body"><Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown></div>
      <div className="plan-artifact-foot">
        <button className="ui-button" data-variant="primary" onClick={onAccept}>Accept & build</button>
        <button className="ui-button" onClick={() => setFbOpen(o => !o)}>{fbOpen ? 'Hide feedback' : 'Request changes'}</button>
        {fbOpen && (
          <div className="plan-feedback">
            <textarea aria-label="What should be different?" placeholder="What's wrong and what do you want different?" value={fb} onChange={e => setFb(e.target.value)} rows={3} />
            <button className="ui-button" data-size="sm" disabled={!fb.trim()} onClick={() => { onFeedback(fb.trim()); setFb(''); setFbOpen(false); }}>Send feedback</button>
          </div>
        )}
      </div>
    </aside>
  );
}
