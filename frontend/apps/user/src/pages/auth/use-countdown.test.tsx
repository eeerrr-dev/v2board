import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useCountdown, type Countdown } from './use-countdown';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('useCountdown', () => {
  let container: HTMLDivElement;
  let root: Root;
  let latest!: Countdown;

  function Harness() {
    latest = useCountdown(60);
    return null;
  }

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => {
      root.render(<Harness />);
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    vi.useRealTimers();
    container.remove();
  });

  it('idles at the sentinel until started', () => {
    expect(latest.remaining).toBe(60);
    expect(latest.isActive).toBe(false);
  });

  it('drops to seconds-1 on start and ticks down each second', () => {
    act(() => latest.start());
    expect(latest.remaining).toBe(59);
    expect(latest.isActive).toBe(true);

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(latest.remaining).toBe(58);
    expect(latest.isActive).toBe(true);
  });

  it('loops back to the sentinel and goes inactive after the full countdown', () => {
    act(() => latest.start());
    // 59 -> 58 -> ... -> 1, then one more tick wraps back to the 60 sentinel.
    for (let i = 0; i < 59; i += 1) {
      act(() => {
        vi.advanceTimersByTime(1000);
      });
    }
    expect(latest.remaining).toBe(60);
    expect(latest.isActive).toBe(false);
  });

  it('clears its timer on unmount without wrapping back', () => {
    act(() => latest.start());
    expect(latest.remaining).toBe(59);
    act(() => root.unmount());
    // No pending timer should survive the unmount to mutate detached state.
    expect(() => {
      act(() => {
        vi.advanceTimersByTime(60000);
      });
    }).not.toThrow();
    expect(latest.remaining).toBe(59);
  });
});
