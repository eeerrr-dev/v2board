// @vitest-environment jsdom
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useTableScrollPosition } from './use-table-scroll-position';

function Harness({ syncOnResize = false }: { syncOnResize?: boolean }) {
  const { bodyRef, onScroll, scrollPosition } = useTableScrollPosition(1, {
    syncOnMount: false,
    syncOnResize,
  });

  return (
    <div ref={bodyRef} data-testid="scroll" data-position={scrollPosition} onScroll={onScroll}>
      <div data-testid="content" />
    </div>
  );
}

function setGeometry(
  node: HTMLElement,
  {
    clientWidth,
    scrollLeft,
    scrollWidth,
  }: Record<'clientWidth' | 'scrollLeft' | 'scrollWidth', number>,
) {
  Object.defineProperties(node, {
    clientWidth: { configurable: true, value: clientWidth },
    scrollLeft: { configurable: true, value: scrollLeft, writable: true },
    scrollWidth: { configurable: true, value: scrollWidth },
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('useTableScrollPosition', () => {
  it('derives left, middle, and right shadow state from the scroll container', () => {
    render(<Harness />);
    const scroll = screen.getByTestId('scroll');

    setGeometry(scroll, { clientWidth: 100, scrollLeft: 0, scrollWidth: 300 });
    fireEvent.scroll(scroll);
    expect(scroll).toHaveAttribute('data-position', 'left');

    scroll.scrollLeft = 50;
    fireEvent.scroll(scroll);
    expect(scroll).toHaveAttribute('data-position', 'middle');

    scroll.scrollLeft = 200;
    fireEvent.scroll(scroll);
    expect(scroll).toHaveAttribute('data-position', 'right');
  });

  it('subscribes to container and content resize and disconnects on unmount', () => {
    const observe = vi.fn();
    const disconnect = vi.fn();
    let notifyResize: ResizeObserverCallback | undefined;

    class ResizeObserverMock {
      constructor(callback: ResizeObserverCallback) {
        notifyResize = callback;
      }

      observe = observe;
      disconnect = disconnect;
    }

    vi.stubGlobal('ResizeObserver', ResizeObserverMock);

    const { unmount } = render(<Harness syncOnResize />);
    const scroll = screen.getByTestId('scroll');
    const content = screen.getByTestId('content');
    expect(observe).toHaveBeenCalledWith(scroll);
    expect(observe).toHaveBeenCalledWith(content);

    setGeometry(scroll, { clientWidth: 100, scrollLeft: 0, scrollWidth: 100 });
    act(() => notifyResize!([], {} as ResizeObserver));
    expect(scroll).toHaveAttribute('data-position', 'both');

    unmount();
    expect(disconnect).toHaveBeenCalledTimes(1);
  });
});
