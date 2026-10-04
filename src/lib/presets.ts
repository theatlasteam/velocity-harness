import type { Provider } from '../types';

export interface Preset extends Omit<Provider, 'key'> {
  hint: string;
  models: string[];
}

const CATALOG_URL = 'https://models.dev/catalog.json?type=all';
const CACHE_KEY = 'models-dev-catalog';
const CACHE_TTL = 24 * 3600 * 1000;

// Vendors in models.dev that have no `api` field (they're consumed via
// AI-SDK packages) but do expose a public OpenAI-compatible endpoint.
const KNOWN_ENDPOINTS: Record<string, { url: string; protocol: 'openai' | 'anthropic' }> = {
  openai: { url: 'https://api.openai.com/v1', protocol: 'openai' },
  anthropic: { url: 'https://api.anthropic.com/v1', protocol: 'anthropic' },
  groq: { url: 'https://api.groq.com/openai/v1', protocol: 'openai' },
  xai: { url: 'https://api.x.ai/v1', protocol: 'openai' },
  mistral: { url: 'https://api.mistral.ai/v1', protocol: 'openai' },
  cerebras: { url: 'https://api.cerebras.ai/v1', protocol: 'openai' },
  cohere: { url: 'https://api.cohere.ai/compatibility/v1', protocol: 'openai' },
  google: { url: 'https://generativelanguage.googleapis.com/v1beta/openai/', protocol: 'openai' },
  deepinfra: { url: 'https://api.deepinfra.com/v1/openai', protocol: 'openai' },
  togetherai: { url: 'https://api.together.xyz/v1', protocol: 'openai' },
  perplexity: { url: 'https://api.perplexity.ai', protocol: 'openai' },
};

const LOCAL_PRESETS: Preset[] = [
  { name: 'Ollama (local)', protocol: 'openai', base_url: 'http://127.0.0.1:11434/v1', model: '', hint: 'no key needed', models: [] },
  { name: 'LM Studio (local)', protocol: 'openai', base_url: 'http://127.0.0.1:1234/v1', model: '', hint: 'local', models: [] },
];

function parseCatalog(raw: any): Preset[] {
  const providers = raw?.providers ?? {};
  const out: Preset[] = [];
  for (const [id, v] of Object.entries<any>(providers)) {
    const api: string | undefined = v?.api || KNOWN_ENDPOINTS[id]?.url;
    if (!api || !/^https?:\/\//.test(api)) continue;
    // Skip Azure: its base URL is per-deployment, not a usable preset.
    if (id === 'azure' || id === 'azure-cognitive-services') continue;
    const protocol = id === 'anthropic' ? 'anthropic' : 'openai';
    const modelIds = Object.keys(v?.models ?? {});
    // Prefer tool-call capable models for the default + list head.
    const withTools = modelIds.filter(m => v.models[m]?.tool_call);
    const ordered = [...withTools, ...modelIds.filter(m => !v.models[m]?.tool_call)];
    out.push({
      name: v?.name || id,
      protocol,
      base_url: api,
      model: ordered[0] || '',
      hint: `${modelIds.length} models · models.dev/${id}`,
      models: ordered,
    });
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  return [...out, ...LOCAL_PRESETS];
}

export async function loadPresets(force = false): Promise<{ presets: Preset[]; fromCache: boolean }> {
  if (!force) {
    try {
      const cached = JSON.parse(localStorage.getItem(CACHE_KEY) || 'null');
      if (cached?.at && Date.now() - cached.at < CACHE_TTL && Array.isArray(cached.presets) && cached.presets.length) {
        return { presets: cached.presets, fromCache: true };
      }
    } catch { /* refetch */ }
  }
  const res = await fetch(CATALOG_URL);
  if (!res.ok) throw new Error(`models.dev: HTTP ${res.status}`);
  const raw = await res.json();
  const presets = parseCatalog(raw);
  try { localStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), presets })); } catch { /* private mode */ }
  return { presets, fromCache: false };
}

export function cachedPresets(): Preset[] {
  try {
    const cached = JSON.parse(localStorage.getItem(CACHE_KEY) || 'null');
    if (Array.isArray(cached?.presets)) return cached.presets;
  } catch { /* ignore */ }
  return [];
}
