import { useEffect, useState } from 'react';

export interface Countdown {
  /** The number to display while counting down (the legacy send-code seconds). */
  remaining: number;
  /** True while the countdown is running (the send-code button stays disabled). */
  isActive: boolean;
  /** Restart the countdown from `seconds - 1`. */
  start: () => void;
}

// Authored V2Board — shared send-code cooldown. Mirrors the legacy 60-sentinel countdown the
// register/forget controllers each duplicated: `seconds` is the idle sentinel, `start()` drops to
// `seconds - 1`, and a 1s setTimeout decrements until it loops back to the sentinel. The effect
// cleans up its timer on unmount so the controllers keep their own mountedRef for post-await work.
export function useCountdown(seconds: number): Countdown {
  const [value, setValue] = useState(seconds);

  useEffect(() => {
    if (value === seconds) return undefined;

    const timer = window.setTimeout(() => {
      setValue((current) => (current <= 1 ? seconds : current - 1));
    }, 1000);

    return () => window.clearTimeout(timer);
  }, [seconds, value]);

  // Plain handler: callers invoke start() from event closures and never read its
  // identity, so the manual useCallback was compiler-redundant.
  const start = () => setValue(seconds - 1);

  return { remaining: value, isActive: value !== seconds, start };
}
