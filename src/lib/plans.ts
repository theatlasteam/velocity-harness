import { invoke, isTauri } from '@tauri-apps/api/core';

export const PLAN_DIR = '/tmp/vhr';
const mem = new Map<string, string>();

export function planNameFor(session: string): string {
  return 'plan-' + session.replace(/[^a-zA-Z0-9_-]/g, '').slice(0, 32);
}

export async function savePlan(name: string, content: string): Promise<string> {
  if (isTauri()) {
    const r = await invoke<{ path: string }>('plan_save', { name, content });
    return r.path;
  }
  mem.set(name, content);
  return `${PLAN_DIR}/${name}.md`;
}

export async function readPlan(name: string): Promise<{ path: string; content: string }> {
  if (isTauri()) return invoke('plan_read', { name });
  const content = mem.get(name) || '';
  return { path: `${PLAN_DIR}/${name}.md`, content };
}
