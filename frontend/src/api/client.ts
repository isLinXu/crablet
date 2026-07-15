import axios from 'axios';
import { LOCAL_STORAGE_KEYS, getApiBaseUrl, isGatewayApiBaseUrl } from '../utils/constants';
import { getSecureItem } from '../utils/secureStorage';
import toast from 'react-hot-toast';

export function getApiErrorMessage(payload: unknown, fallback = '请求失败'): string {
  if (typeof payload === 'string') return payload.trim() || fallback;
  if (!payload || typeof payload !== 'object') return fallback;
  const data = payload as Record<string, unknown>;
  const nested = data.error && typeof data.error === 'object' ? data.error as Record<string, unknown> : undefined;
  for (const value of [data.message, data.detail, nested?.message, data.error]) {
    if (typeof value === 'string' && value.trim()) return value;
  }
  return fallback;
}

const client = axios.create({
  baseURL: getApiBaseUrl(),
  timeout: 120000, // Allow local models time to respond; streaming uses fetch cancellation
  headers: {
    'Content-Type': 'application/json',
  },
});

// Request interceptor
client.interceptors.request.use(
  (config) => {
    config.baseURL = getApiBaseUrl();
    const token = getSecureItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
    const apiKey = getSecureItem(LOCAL_STORAGE_KEYS.API_KEY);
    const shouldSendAuth = isGatewayApiBaseUrl(config.baseURL);
    if (token && shouldSendAuth) {
      config.headers.Authorization = `Bearer ${token}`;
    } else if (apiKey && shouldSendAuth) {
      config.headers.Authorization = `Bearer ${apiKey}`;
    } else {
      if (config.headers?.Authorization) {
        delete config.headers.Authorization;
      }
    }
    if (import.meta.env.DEV) {
      console.debug(`[API Request] ${config.method?.toUpperCase()} ${config.baseURL}${config.url}`);
    }
    return config;
  },
  (error) => {
    return Promise.reject(error);
  }
);

// Response interceptor
client.interceptors.response.use(
  (response) => {
    return response;
  },
  (error) => {
    let message = '';
    if (error.code === 'ERR_NETWORK') {
      console.error('Network Error Details:', {
        url: error.config?.url,
        baseURL: error.config?.baseURL,
        method: error.config?.method,
        headers: error.config?.headers
      });
      message = `无法连接到服务器 (${error.config?.baseURL})，请检查网络设置或API地址配置。`;
    } else {
      message = getApiErrorMessage(error.response?.data, error.message || '请求失败');
    }
    
    // Handle 401 Unauthorized
    if (error.response?.status === 401) {
      if (isGatewayApiBaseUrl(error.config?.baseURL)) {
        localStorage.removeItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
      }
      toast.error('Session expired or Unauthorized. Please check your settings.');
    } 
    // Handle 429 Too Many Requests
    else if (error.response?.status === 429) {
      message = 'Too many requests. Please try again later.';
      toast.error(message);
    }
    // Handle 5xx Server Errors
    else if (error.response?.status >= 500) {
      message = `Server Error (${error.response.status}). Please try again later.`;
      toast.error(message);
    }
    else {
      toast.error(message);
    }
    
    return Promise.reject(error);
  }
);

export default client;
