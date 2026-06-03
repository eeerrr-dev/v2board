const AUTH_KEY = 'authorization';

type Listener = (value: string | null) => void;
const listeners = new Set<Listener>();

export function getAuthData(): string | null {
  return localStorage.getItem(AUTH_KEY);
}

export function setAuthData(value: string | null): void {
  if (value === null) localStorage.removeItem(AUTH_KEY);
  else localStorage.setItem(AUTH_KEY, value);
  for (const l of listeners) l(value);
}

export function subscribeAuth(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setupAuthSync(): void {
  window.addEventListener('storage', (event) => {
    if (event.key === AUTH_KEY) {
      for (const l of listeners) l(event.newValue);
    }
  });
}

export function logout(): void {
  setAuthData(null);
}
