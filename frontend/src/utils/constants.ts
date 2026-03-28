export const LOCAL_STORAGE_KEYS = {
  THEME: 'crablet-theme',
  AUTH_TOKEN: 'crablet-token',
  API_BASE_URL: 'crablet-api-base-url',
  API_KEY: 'crablet-api-key',
  CANVAS_STATE: 'crablet-canvas',
  SESSION_ID: 'crablet-session-id',
  MODEL_PRIORITY: 'crablet-model-priority',
};

const getDefaultApiFallback = () => {
  if (typeof window === 'undefined' || !window.location) {
    return 'http://127.0.0.1:18789/api';
  }

  const { protocol, hostname, port } = window.location;

  // Vite dev server uses proxy for /api requests.
  if (port === '5173') {
    return '/api';
  }

  const apiProtocol = protocol === 'https:' ? 'https:' : 'http:';
  return `${apiProtocol}//${hostname}:18789/api`;
};

export const getApiBaseUrl = () => {
  const fallback = getDefaultApiFallback();
  let url = (localStorage.getItem(LOCAL_STORAGE_KEYS.API_BASE_URL) || import.meta.env.VITE_API_URL || fallback).trim();

  if (!url) {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, fallback);
    return fallback;
  }

  // Auto-fix if pointing to serve-web (3333/3000) or the stale 18790 gateway port.
  if (url === '/api' || url.includes(':3333') || url.includes(':3000') || url.includes(':18790')) {
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
    const currentHostname = typeof window !== 'undefined' ? window.location.hostname : '';
    const parsedIsLoopback = parsed.hostname === '127.0.0.1' || parsed.hostname === 'localhost';
    const currentIsLoopback = currentHostname === '127.0.0.1' || currentHostname === 'localhost';
    if (parsedIsLoopback && currentHostname && !currentIsLoopback) {
      parsed.hostname = currentHostname;
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

const getDefaultWsBase = () => {
  if (typeof window === 'undefined' || !window.location) {
    return 'ws://127.0.0.1:18789';
  }

  const apiBaseUrl = getApiBaseUrl();
  if (apiBaseUrl.startsWith('/')) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}`;
  }

  try {
    const parsed = new URL(apiBaseUrl);
    parsed.protocol = parsed.protocol === 'https:' ? 'wss:' : 'ws:';
    parsed.pathname = '';
    parsed.search = '';
    parsed.hash = '';
    return parsed.toString().replace(/\/$/, '');
  } catch {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}`;
  }
};

export const getWsUrl = (path = '/ws') => {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const configured = import.meta.env.VITE_WS_URL?.trim();
  const wsBase = configured || getDefaultWsBase();

  if (/^wss?:\/\//i.test(wsBase)) {
    const parsed = new URL(wsBase);
    parsed.pathname = normalizedPath;
    parsed.search = '';
    parsed.hash = '';
    return parsed.toString();
  }

  return `${wsBase.replace(/\/+$/, '')}${normalizedPath}`;
};

export const ROUTES = {
  HOME: '/',
  CHAT: '/chat',
  KNOWLEDGE: '/knowledge',
  SETTINGS: '/settings',
};
