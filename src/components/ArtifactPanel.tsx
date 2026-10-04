import React, { useState } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export default function ArtifactPanel({ name, path, content, onClose }: {
  name: string;
  path?: string;
  content: string;
  onClose: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const isMd = /\.md(own)?$|markdown/i.test(name);
  const copy = () => {
    navigator.clipboard.writeText(content).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }).catch(() => {});
  };
  const download = () => {
    const blob = new Blob([content], { type: 'text/plain' });
    const u = URL.createObjectURL(blob);
    const l = document.createElement('a');
    l.href = u;
    l.download = name;
    l.click();
    setTimeout(() => URL.revokeObjectURL(u), 2000);
  };
  return (
    <aside className="artifact-panel" aria-label="Artifact">
      <div className="artifact-panel-head">
        <div><strong>{name}</strong>{path && <small>{path}</small>}</div>
        <button className="icon-button" onClick={onClose} aria-label="Close artifact">✕</button>
      </div>
      <div className="artifact-panel-body">
        {isMd ? <Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown> : <pre>{content}</pre>}
      </div>
      <div className="artifact-panel-foot">
        <button className="ui-button" data-size="sm" onClick={copy}>{copied ? 'Copied' : 'Copy'}</button>
        <button className="ui-button" data-size="sm" onClick={download}>Download</button>
      </div>
    </aside>
  );
}
