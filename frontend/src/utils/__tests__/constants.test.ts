import { describe, expect, it, beforeEach } from 'vitest';
import { getApiBaseUrl, LOCAL_STORAGE_KEYS } from '../constants';

describe('getApiBaseUrl', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('falls back to /api when malformed url is stored', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http:///api');
    expect(getApiBaseUrl()).toBe('/api');
  });

  it('normalizes localhost to 127.0.0.1', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http://localhost:18789/api');
    expect(getApiBaseUrl()).toBe('http://127.0.0.1:18789/api');
  });

  it('normalizes relative api path', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, '/api/');
    expect(getApiBaseUrl()).toBe('/api');
  });

  it('normalizes bare api to relative path', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'api');
    expect(getApiBaseUrl()).toBe('/api');
  });

  it('falls back to /api for suspicious host http://api', () => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, 'http://api');
    expect(getApiBaseUrl()).toBe('/api');
  });
});
