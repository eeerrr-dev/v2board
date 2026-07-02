import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useCountdown } from './use-countdown';

describe('useCountdown', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('idles at the sentinel until started', () => {
    const { result } = renderHook(() => useCountdown(60));

    expect(result.current.remaining).toBe(60);
    expect(result.current.isActive).toBe(false);
  });

  it('drops to seconds-1 on start and ticks down each second', () => {
    const { result } = renderHook(() => useCountdown(60));

    act(() => {
      result.current.start();
    });
    expect(result.current.remaining).toBe(59);
    expect(result.current.isActive).toBe(true);

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(result.current.remaining).toBe(58);
    expect(result.current.isActive).toBe(true);
  });

  it('loops back to the sentinel and goes inactive after the full countdown', () => {
    const { result } = renderHook(() => useCountdown(60));

    act(() => {
      result.current.start();
    });
    // 59 -> 58 -> ... -> 1, then one more tick wraps back to the 60 sentinel.
    for (let i = 0; i < 59; i += 1) {
      act(() => {
        vi.advanceTimersByTime(1000);
      });
    }
    expect(result.current.remaining).toBe(60);
    expect(result.current.isActive).toBe(false);
  });

  it('clears its timer on unmount without wrapping back', () => {
    const { result, unmount } = renderHook(() => useCountdown(60));

    act(() => {
      result.current.start();
    });
    expect(result.current.remaining).toBe(59);

    unmount();
    // No pending timer should survive the unmount to mutate detached state.
    expect(() => {
      act(() => {
        vi.advanceTimersByTime(60000);
      });
    }).not.toThrow();
    expect(result.current.remaining).toBe(59);
  });
});
