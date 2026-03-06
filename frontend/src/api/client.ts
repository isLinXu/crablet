import axios from 'axios';
import { LOCAL_STORAGE_KEYS, getApiBaseUrl } from '../utils/constants';
import toast from 'react-hot-toast';

const isGatewayLocalApi = (baseURL?: string) => {
  if (!baseURL) return true;
  if (baseURL.startsWith('/api')) return true;
  return /127\.0\.0\.1:18789\/api/.test(baseURL) || /localhost:18789\/api/.test(baseURL);
};

const client = axios.create({
  baseURL: getApiBaseUrl(),
  timeout: 60000, // Increased to 60s for local RAG/MCP operations
  headers: {
    'Content-Type': 'application/json',
  },
});

// Request interceptor
client.interceptors.request.use(
  (config) => {
    config.baseURL = getApiBaseUrl();
    const token = localStorage.getItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
    const apiKey = localStorage.getItem(LOCAL_STORAGE_KEYS.API_KEY);
    const shouldSendAuth = isGatewayLocalApi(config.baseURL);
    if (token && shouldSendAuth) {
      config.headers.Authorization = `Bearer ${token}`;
    } else if (apiKey && shouldSendAuth) {
      config.headers.Authorization = `Bearer ${apiKey}`;
    } else {
      if (config.headers?.Authorization) {
        delete config.headers.Authorization;
      }
    }
    console.log(`[API Request] ${config.method?.toUpperCase()} ${config.baseURL}${config.url}`, config);
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
      message = error.response?.data?.message || error.message || 'An error occurred';
    }
    
    // Handle 401 Unauthorized
    if (error.response?.status === 401) {
      if (isGatewayLocalApi(error.config?.baseURL)) {
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
