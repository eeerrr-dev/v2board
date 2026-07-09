import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';

type ScrollPosition = 'left' | 'right' | 'middle' | 'both';

interface TableScrollPositionOptions {
  syncOnMount?: boolean;
  syncOnResize?: boolean;
}

export function useTableScrollPosition(
  rowCount: number,
  { syncOnMount = true, syncOnResize = true }: TableScrollPositionOptions = {},
) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const [position, setPosition] = useState<ScrollPosition>('left');

  const compute = useCallback(() => {
    const node = bodyRef.current;
    if (!node) return;
    // `scrollWidth - clientWidth` is the maximum scrollLeft, so comparing it to the
    // current scrollLeft detects the right edge without any getBoundingClientRect
    // reads or the legacy +1 sub-pixel fudge.
    const atLeft = node.scrollLeft <= 0;
    const atRight = node.scrollWidth - node.clientWidth - node.scrollLeft <= 0;
    setPosition((prev) => {
      if (atLeft && atRight) return 'both';
      if (atLeft) return 'left';
      if (atRight) return 'right';
      return prev === 'middle' ? prev : 'middle';
    });
  }, []);

  useLayoutEffect(() => {
    if (syncOnMount) compute();
  }, [compute, rowCount, syncOnMount]);

  useEffect(() => {
    if (!syncOnResize) return undefined;
    const node = bodyRef.current;
    if (!node) return undefined;
    // Observe the scroll container and its content directly so the shadow state
    // tracks any size change — viewport resize, sidebar toggle, font load, content
    // reflow — not just window 'resize' events. ResizeObserver coalesces its
    // callbacks through the browser, replacing the legacy 150ms trailing debounce.
    const observer = new ResizeObserver(() => compute());
    observer.observe(node);
    const inner = node.children[0];
    if (inner) observer.observe(inner);
    return () => observer.disconnect();
  }, [compute, rowCount, syncOnResize]);

  return { bodyRef, onScroll: compute, scrollPosition: position };
}
