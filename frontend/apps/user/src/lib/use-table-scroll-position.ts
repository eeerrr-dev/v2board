import { useCallback, useRef, useState, useSyncExternalStore } from 'react';

type ScrollPosition = 'left' | 'right' | 'middle' | 'both';

interface TableScrollPositionOptions {
  syncOnMount?: boolean;
  syncOnResize?: boolean;
}

const getServerScrollPosition = (): ScrollPosition => 'left';

function createScrollPositionStore() {
  let position: ScrollPosition = 'left';
  const listeners = new Set<() => void>();

  return {
    getSnapshot: () => position,
    publish(nextPosition: ScrollPosition) {
      if (position === nextPosition) return;
      position = nextPosition;
      listeners.forEach((listener) => listener());
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export function useTableScrollPosition(
  rowCount: number,
  { syncOnMount = true, syncOnResize = true }: TableScrollPositionOptions = {},
) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const [store] = useState(createScrollPositionStore);

  const compute = useCallback(() => {
    const node = bodyRef.current;
    if (!node) return;
    // `scrollWidth - clientWidth` is the maximum scrollLeft, so comparing it to the
    // current scrollLeft detects the right edge without any getBoundingClientRect
    // reads or a manual +1 sub-pixel fudge.
    const atLeft = node.scrollLeft <= 0;
    const atRight = node.scrollWidth - node.clientWidth - node.scrollLeft <= 0;
    const nextPosition =
      atLeft && atRight ? 'both' : atLeft ? 'left' : atRight ? 'right' : 'middle';
    store.publish(nextPosition);
  }, [store]);

  const subscribe = useCallback(
    (listener: () => void) => {
      // Treat the row count as the content version: a change re-subscribes and
      // re-observes the first child even when the container node is unchanged.
      void rowCount;
      const node = bodyRef.current;
      const unsubscribe = store.subscribe(listener);
      if (syncOnMount) compute();

      let observer: ResizeObserver | undefined;
      if (syncOnResize && node) {
        // Observe the scroll container and its content directly so the shadow
        // state tracks viewport, sidebar, font, and content reflow changes.
        observer = new ResizeObserver(() => compute());
        observer.observe(node);
        const inner = node.children[0];
        if (inner) observer.observe(inner);
      }

      return () => {
        unsubscribe();
        observer?.disconnect();
      };
    },
    [compute, rowCount, store, syncOnMount, syncOnResize],
  );

  const position = useSyncExternalStore(subscribe, store.getSnapshot, getServerScrollPosition);

  return { bodyRef, onScroll: compute, scrollPosition: position };
}
