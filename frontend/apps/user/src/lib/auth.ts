const AUTH_STORAGE_KEY = 'authorization';
const SUBSCRIBE_TOKEN_KEY = 'subscribe_token';

type AuthListener = (value: string | null) => void;
const listeners = new Set<AuthListener>();

export function getAuthData(): string | null {
  return localStorage.getItem(AUTH_STORAGE_KEY);
}

export function setAuthData(value: string | null): void {
  if (value === null) {
    localStorage.removeItem(AUTH_STORAGE_KEY);
  } else {
    localStorage.setItem(AUTH_STORAGE_KEY, value);
  }
  for (const l of listeners) l(value);
}

export function subscribeAuth(listener: AuthListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getSubscribeToken(): string | null {
  return localStorage.getItem(SUBSCRIBE_TOKEN_KEY);
}

export function setSubscribeToken(value: string | null): void {
  if (value === null) localStorage.removeItem(SUBSCRIBE_TOKEN_KEY);
  else localStorage.setItem(SUBSCRIBE_TOKEN_KEY, value);
}

export function setupAuthSync(): void {
  window.addEventListener('storage', (event) => {
    if (event.key === AUTH_STORAGE_KEY) {
      for (const l of listeners) l(event.newValue);
    }
  });
}

export function logout(): void {
  setAuthData(null);
  setSubscribeToken(null);
}
