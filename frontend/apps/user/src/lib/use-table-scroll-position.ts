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
    const inner = node?.children[0] as HTMLElement | undefined;
    if (!node || !inner) return;
    const atLeft = node.scrollLeft === 0;
    const atRight =
      node.scrollLeft + 1 >=
      inner.getBoundingClientRect().width - node.getBoundingClientRect().width;
    setPosition((prev) => {
      if (atLeft && atRight) return 'both';
      if (atLeft) return 'left';
      if (atRight) return 'right';
      return prev === 'middle' ? prev : 'middle';
    });
  }, []);

  useLayoutEffect(() => {
    if (syncOnMount) compute();
  });

  useEffect(() => {
    if (!syncOnResize) return undefined;
    // The original routes resize through a 150ms trailing debounce
    // (_.debounce(handleWindowResize, 150)) and cancels it on unmount.
    let timer: number | undefined;
    const onResize = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(compute, 150);
    };
    window.addEventListener('resize', onResize);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener('resize', onResize);
    };
  }, [compute, rowCount, syncOnResize]);

  return { bodyRef, onScroll: compute, scrollPosition: position };
}
