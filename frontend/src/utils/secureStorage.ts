/**
 * Secure storage for sensitive credentials (API keys, auth tokens).
 *
 * Uses sessionStorage instead of localStorage so that credentials:
 * - Are scoped to the current tab/session (not shared across tabs)
 * - Are cleared when the tab closes (not persisted to disk)
 * - Are not accessible to other origins/tabs
 *
 * Non-sensitive config (API base URL, theme, etc.) stays in localStorage.
 */

type StorageType = 'session' | 'local';

const SENSITIVE_KEYS = new Set<string>([
  'crablet-api-key',
  'crablet-token',
]);

function getStorage(type: StorageType): Storage | null {
  try {
    return type === 'session' ? sessionStorage : localStorage;
  } catch {
    return null;
  }
}

export function getSecureItem(key: string): string | null {
  const storageType: StorageType = SENSITIVE_KEYS.has(key) ? 'session' : 'local';
  const storage = getStorage(storageType);
  if (!storage) return null;

  // For sensitive keys, try sessionStorage first, then fall back to localStorage
  // (for migration from older versions that stored in localStorage)
  if (storageType === 'session') {
    const sessionValue = sessionStorage.getItem(key);
    if (sessionValue) return sessionValue;

    // Migration: check if value exists in localStorage and move it to sessionStorage
    const localValue = localStorage.getItem(key);
    if (localValue) {
      sessionStorage.setItem(key, localValue);
      localStorage.removeItem(key);
      return localValue;
    }
    return null;
  }

  return storage.getItem(key);
}

export function setSecureItem(key: string, value: string): void {
  const storageType: StorageType = SENSITIVE_KEYS.has(key) ? 'session' : 'local';
  const storage = getStorage(storageType);
  if (!storage) return;

  if (storageType === 'session') {
    sessionStorage.setItem(key, value);
    // Also remove from localStorage if it was there (migration)
    localStorage.removeItem(key);
  } else {
    storage.setItem(key, value);
  }
}

export function removeSecureItem(key: string): void {
  // Remove from both storages to ensure complete cleanup
  sessionStorage.removeItem(key);
  localStorage.removeItem(key);
}
