export const LOCAL_STORAGE_KEYS = {
  THEME: 'crablet-theme',
  AUTH_TOKEN: 'crablet-token',
  API_BASE_URL: 'crablet-api-base-url',
  API_KEY: 'crablet-api-key',
  CANVAS_STATE: 'crablet-canvas',
  SESSION_ID: 'crablet-session-id',
  MODEL_PRIORITY: 'crablet-model-priority',
};

export const getApiBaseUrl = () => {
  const fallback = '/api';
  let url = (localStorage.getItem(LOCAL_STORAGE_KEYS.API_BASE_URL) || import.meta.env.VITE_API_URL || fallback).trim();

  if (!url) {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, fallback);
    return fallback;
  }

  if (url.includes('dashscope.aliyuncs.com') || url.includes('api.openai.com') || url.includes('anthropic.com')) {
    console.warn('Detected invalid Backend URL (points to LLM provider). Resetting to default.');
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, fallback);
    return fallback;
  }

  if (/^api(?:\/.*)?$/i.test(url)) {
    const normalized = `/${url.replace(/^\/+/, '')}`.replace(/\/+$/, '') || fallback;
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, normalized);
    return normalized;
  }

  if (url.startsWith('/')) {
    const normalized = url.replace(/\/+$/, '') || fallback;
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, normalized);
    return normalized;
  }

  if (!/^https?:\/\//i.test(url)) {
    url = `http://${url}`;
  }
  if (/^https?:\/\/\//i.test(url)) {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, fallback);
    return fallback;
  }

  try {
    const parsed = new URL(url);
    if (!parsed.hostname) {
      localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, fallback);
      return fallback;
    }
    if (parsed.hostname === 'localhost') {
      parsed.hostname = '127.0.0.1';
    }
    if (parsed.hostname === 'api' && !parsed.port && (!parsed.pathname || parsed.pathname === '/')) {
      localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, fallback);
      return fallback;
    }
    parsed.pathname = parsed.pathname.replace(/\/+$/, '');
    const normalized = parsed.toString().replace(/\/$/, '');
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, normalized);
    return normalized;
  } catch {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, fallback);
    return fallback;
  }
};

export const getWsUrl = () => {
  // If we are proxying, use window.location.host
  // But if we have a custom API URL, we should probably use that host?
  // For now, let's just make sure localhost -> 127.0.0.1 if explicitly set
  const wsUrl = import.meta.env.VITE_WS_URL || (window.location.protocol === 'https:' ? 'wss://' : 'ws://') + window.location.host + '/ws';
  // Do NOT force 127.0.0.1 for WS if we are using the proxy (port 5173)
  // The browser needs to connect to the same host as the page to avoid CORS/Origin issues with the proxy.
  return wsUrl;
};

export const ROUTES = {
  HOME: '/',
  CHAT: '/chat',
  KNOWLEDGE: '/knowledge',
  SETTINGS: '/settings',
};
