// @vitest-environment jsdom
import { act, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useIsMobile } from './use-mobile';

function Harness() {
  return <span>{useIsMobile() ? 'mobile' : 'desktop'}</span>;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('useIsMobile', () => {
  it('reads the current viewport snapshot and unsubscribes from media-query changes', () => {
    let listener: EventListener | undefined;
    const removeEventListener = vi.fn();
    const matchMedia = vi.fn(
      () =>
        ({
          matches: false,
          media: '(max-width: 767px)',
          onchange: null,
          addEventListener: vi.fn((_type: string, callback: EventListenerOrEventListenerObject) => {
            if (typeof callback === 'function') listener = callback;
          }),
          removeEventListener,
          addListener: vi.fn(),
          removeListener: vi.fn(),
          dispatchEvent: vi.fn(),
        }) as unknown as MediaQueryList,
    );
    vi.stubGlobal('innerWidth', 1024);
    vi.stubGlobal('matchMedia', matchMedia);

    const { unmount } = render(<Harness />);
    expect(screen.getByText('desktop')).toBeInTheDocument();
    expect(matchMedia).toHaveBeenCalledWith('(max-width: 767px)');

    vi.stubGlobal('innerWidth', 500);
    act(() => listener?.(new Event('change')));
    expect(screen.getByText('mobile')).toBeInTheDocument();

    unmount();
    expect(removeEventListener).toHaveBeenCalledTimes(1);
  });
});
