// After a deploy, an already-open tab can hold an index.html whose lazy route
// imports point at hashed chunks the new release no longer ships (the deploy
// root's `previous` fallback covers one in-flight rollout, not a tab that
// slept through two). Vite surfaces those failures as a cancelable
// `vite:preloadError` window event; one forced reload fetches the current
// index.html and its live chunk graph. The sessionStorage timestamp is the
// loop guard: within the cooldown the error propagates to the router's error
// boundary instead of reloading again, so a genuinely broken release can
// never reload-loop the tab.

const RELOADED_AT_KEY = 'v2board:chunk-recovery-reloaded-at';
const RELOAD_COOLDOWN_MS = 30_000;

function readLastReloadAt(): number | null {
  try {
    return Number(window.sessionStorage.getItem(RELOADED_AT_KEY) ?? 0);
  } catch {
    return null;
  }
}

export function installChunkReloadRecovery(
  reload: () => void = () => window.location.reload(),
): () => void {
  let reloading = false;
  const onPreloadError = (event: Event) => {
    if (reloading) {
      // The recovery reload is already in flight; swallow follow-up failures
      // (e.g. the same route's CSS) instead of flashing an error boundary.
      event.preventDefault();
      return;
    }
    const lastReloadAt = readLastReloadAt();
    if (lastReloadAt === null || Date.now() - lastReloadAt < RELOAD_COOLDOWN_MS) return;
    try {
      window.sessionStorage.setItem(RELOADED_AT_KEY, String(Date.now()));
    } catch {
      // Without a working loop guard, auto-reloading is unsafe; let the
      // error surface.
      return;
    }
    reloading = true;
    event.preventDefault();
    reload();
  };
  window.addEventListener('vite:preloadError', onPreloadError);
  return () => window.removeEventListener('vite:preloadError', onPreloadError);
}
