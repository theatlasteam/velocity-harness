import React, { useState } from 'react';

export default function PlanReview({ onAccept, onFeedback, disabled }: {
  onAccept: () => void;
  onFeedback: (text: string) => void;
  disabled: boolean;
}) {
  const [fb, setFb] = useState('');
  const [open, setOpen] = useState(false);
  return (
    <div className="plan-review" data-testid="plan-review">
      <div className="plan-review-head"><strong>Review this plan</strong><span>Accept to let it build, or request changes.</span></div>
      <div className="plan-review-actions">
        <button className="ui-button" data-variant="primary" disabled={disabled} onClick={onAccept}>Accept & build</button>
        <button className="ui-button" disabled={disabled} onClick={() => setOpen(o => !o)}>{open ? 'Hide feedback' : 'Request changes'}</button>
      </div>
      {open && (
        <div className="plan-feedback">
          <textarea aria-label="What should be different?" placeholder="What's wrong and what do you want different?" value={fb} onChange={e => setFb(e.target.value)} rows={3} disabled={disabled} />
          <button className="ui-button" data-size="sm" disabled={disabled || !fb.trim()} onClick={() => { onFeedback(fb.trim()); setFb(''); setOpen(false); }}>Send feedback</button>
        </div>
      )}
    </div>
  );
}
