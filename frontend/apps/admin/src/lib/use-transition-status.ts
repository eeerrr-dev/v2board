import { useEffect, useState } from 'react';

export type TransitionStatus = 'enter' | 'entering' | 'entered' | 'leave' | 'leaving' | 'exited';

// Drives an enter/leave animation the way antd's rc-animate does. On enter it
// mounts with the base class ("enter"), then adds the "-active" class so the
// animation runs ("entering"), then removes both classes ("entered"); on leave
// it applies the base class ("leave"), then the "-active" class ("leaving")
// before unmounting after `duration`.
//
// rc-animate's css-animation core adds the "-active" class after a fixed 30ms
// paused hold (setTimeout(...,30)) for both enter and leave. Pass `holdMs` (30)
// to reproduce that for CSS animation consumers.
export function useTransitionStatus(
  open: boolean,
  duration: number,
  holdMs?: number,
): TransitionStatus {
  const [status, setStatus] = useState<TransitionStatus>(open ? 'entered' : 'exited');

  useEffect(() => {
    if (open) {
      setStatus((prev) => (prev === 'entered' ? prev : 'enter'));
      return;
    }
    setStatus((prev) => (prev === 'exited' ? prev : 'leave'));
    const timer = window.setTimeout(() => setStatus('exited'), duration);
    return () => window.clearTimeout(timer);
  }, [open, duration]);

  useEffect(() => {
    if (status !== 'enter') return;
    if (holdMs != null) {
      const timer = window.setTimeout(() => setStatus('entering'), holdMs);
      return () => window.clearTimeout(timer);
    }
    const raf = requestAnimationFrame(() => setStatus('entered'));
    return () => cancelAnimationFrame(raf);
  }, [status, holdMs]);

  useEffect(() => {
    if (status !== 'entering') return;
    const remaining = Math.max(duration - (holdMs ?? 0), 0);
    const timer = window.setTimeout(() => setStatus('entered'), remaining);
    return () => window.clearTimeout(timer);
  }, [status, duration, holdMs]);

  useEffect(() => {
    if (status !== 'leave') return;
    if (holdMs != null) {
      const timer = window.setTimeout(() => setStatus('leaving'), holdMs);
      return () => window.clearTimeout(timer);
    }
    const raf = requestAnimationFrame(() => setStatus('leaving'));
    return () => cancelAnimationFrame(raf);
  }, [status, holdMs]);

  return status;
}
