import React, { useState } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export interface Artifact { lang: string; code: string; index: number }

export function extractArtifacts(text: string): Artifact[] {
  const out: Artifact[] = [];
  const re = /```(html|markdown|md|svg)\n([\s\S]*?)```/gi;
  let m: RegExpExecArray | null;
  let i = 0;
  while ((m = re.exec(text))) {
    if (m[2].trim().length < 10) continue;
    out.push({ lang: m[1].toLowerCase() === 'md' ? 'markdown' : m[1].toLowerCase(), code: m[2].trim(), index: i++ });
    if (out.length >= 4) break;
  }
  return out;
}

function ArtifactCard({ artifact }: { artifact: Artifact }) {
  const [tab, setTab] = useState<'preview' | 'code'>('preview');
  const [copied, setCopied] = useState(false);
  const title = artifact.lang === 'html' ? 'HTML preview' : artifact.lang === 'svg' ? 'SVG preview' : 'Markdown preview';
  const copy = () => {
    navigator.clipboard.writeText(artifact.code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }).catch(() => {});
  };
  const open = () => {
    const blob = new Blob([artifact.code], { type: 'text/html' });
    window.open(URL.createObjectURL(blob), '_blank');
  };
  return (
    <div className="artifact-card" data-artifact={artifact.lang}>
      <div className="artifact-head">
        <span className="artifact-title">{title}</span>
        <div className="artifact-tabs" role="tablist">
          {(['preview', 'code'] as const).map(t => (
            <button key={t} role="tab" aria-selected={tab === t} className={tab === t ? 'active' : ''} onClick={() => setTab(t)}>{t === 'preview' ? 'Preview' : 'Code'}</button>
          ))}
        </div>
        <div className="artifact-actions">
          <button className="text-button artifact-btn" onClick={copy}>{copied ? 'Copied' : 'Copy'}</button>
          {artifact.lang !== 'markdown' && <button className="text-button artifact-btn" onClick={open}>Open</button>}
        </div>
      </div>
      {tab === 'preview' ? (
        artifact.lang === 'markdown' ? (
          <div className="artifact-preview artifact-md"><Markdown remarkPlugins={[remarkGfm]}>{artifact.code}</Markdown></div>
        ) : (
          <iframe className="artifact-preview" title={title} sandbox="allow-scripts" srcDoc={artifact.code} />
        )
      ) : (
        <pre className="artifact-code"><code>{artifact.code.slice(0, 8000)}</code></pre>
      )}
    </div>
  );
}

export default function ArtifactBlock({ text }: { text: string }) {
  const arts = extractArtifacts(text);
  if (!arts.length) return null;
  return <div className="artifact-list">{arts.map(a => <ArtifactCard key={a.index} artifact={a} />)}</div>;
}
