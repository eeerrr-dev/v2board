import { useSyncExternalStore } from 'react';

const MOBILE_BREAKPOINT = 768;

const MOBILE_MEDIA_QUERY = `(max-width: ${MOBILE_BREAKPOINT - 1}px)`;

function subscribeMobile(callback: () => void): () => void {
  const media = window.matchMedia(MOBILE_MEDIA_QUERY);
  media.addEventListener('change', callback);
  return () => media.removeEventListener('change', callback);
}

function getMobileSnapshot(): boolean {
  return window.innerWidth < MOBILE_BREAKPOINT;
}

// Viewport width is an external browser store, not component-owned state.
// useSyncExternalStore gives render a current, tear-free snapshot and keeps the
// media-query subscription lifecycle out of a state-mirroring effect.
export function useIsMobile() {
  return useSyncExternalStore(subscribeMobile, getMobileSnapshot, () => false);
}
