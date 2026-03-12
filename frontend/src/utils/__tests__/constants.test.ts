import { describe, expect, it, beforeEach } from 'vitest';
import { getApiBaseUrl, LOCAL_STORAGE_KEYS } from '../constants';

describe('getApiBaseUrl', () => {
  const getExpectedFallback = () => {
    if (window.location.port === '5173') return '/api';
    const apiProtocol = window.location.protocol === 'https:' ? 'https:' : 'http:';
    return `${apiProtocol}//${window.location.hostname}:18789/api`;
  };

  beforeEach(() => {
    localStorage.clear();
  });

  it('falls back to default gateway url when malformed url is stored', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http:///api');
    expect(getApiBaseUrl()).toBe(getExpectedFallback());
  });

  it('keeps explicit localhost url unchanged', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http://localhost:18789/api');
    expect(getApiBaseUrl()).toBe('http://localhost:18789/api');
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
});
