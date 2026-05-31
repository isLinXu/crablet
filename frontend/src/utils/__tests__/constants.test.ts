import { describe, expect, it, beforeEach } from 'vitest';
import { getApiBaseUrl, isGatewayApiBaseUrl, LOCAL_STORAGE_KEYS, normalizeApiRequestPath, resolveApiUrl } from '../constants';

describe('getApiBaseUrl', () => {
  const getExpectedFallback = () => {
    if (window.location.port === '5173') return '/api';
    const apiProtocol = window.location.protocol === 'https:' ? 'https:' : 'http:';
    return `${apiProtocol}//${window.location.hostname}:18790/api`;
  };

  beforeEach(() => {
    localStorage.clear();
  });

  it('falls back to default gateway url when malformed url is stored', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http:///api');
    expect(getApiBaseUrl()).toBe(getExpectedFallback());
  });

  it('keeps explicit localhost url unchanged', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http://localhost:18790/api');
    expect(getApiBaseUrl()).toBe('http://localhost:18790/api');
  });

  it('normalizes relative api path', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, '/api/');
    expect(getApiBaseUrl()).toBe('/api');
  });

  it('normalizes bare api to relative path', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'api');
    expect(getApiBaseUrl()).toBe('/api');
  });

  it('falls back to default gateway url for suspicious host http://api', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http://api');
    expect(getApiBaseUrl()).toBe(getExpectedFallback());
  });

  it('migrates stale 18789 gateway url to the current default gateway', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http://localhost:18789/api');
    expect(getApiBaseUrl()).toBe(getExpectedFallback());
  });

  it('detects gateway base urls by api prefix instead of hard-coded legacy port checks', () => {
    expect(isGatewayApiBaseUrl('/api')).toBe(true);
    expect(isGatewayApiBaseUrl('http://localhost:18790/api')).toBe(true);
    expect(isGatewayApiBaseUrl('https://gateway.example.com/api')).toBe(true);
    expect(isGatewayApiBaseUrl('https://api.openai.com/v1')).toBe(false);
  });

  it('normalizes legacy api-prefixed request paths without duplicating the api segment', () => {
    expect(normalizeApiRequestPath('/api/v1/chat')).toBe('/v1/chat');
    expect(normalizeApiRequestPath('/v1/chat')).toBe('/v1/chat');
  });

  it('resolves streaming and swarm endpoints through the configured api base url', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'https://gateway.example.com/api');
    expect(resolveApiUrl('/v1/chat/stream')).toBe('https://gateway.example.com/api/v1/chat/stream');
    expect(resolveApiUrl('/api/v1/swarm/stats')).toBe('https://gateway.example.com/api/v1/swarm/stats');
  });
});
