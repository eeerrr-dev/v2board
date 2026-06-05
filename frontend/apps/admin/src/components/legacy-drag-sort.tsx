import {
  useEffect,
  useRef,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';

const LEGACY_DRAG_LINE_STYLE =
  'position:fixed;z-index:9999;height:0;margin-top:-1px;border-bottom:dashed 2px rgba(0,0,0,.3);display:none;';

function findClosestWithin(target: EventTarget | null, selector: string, root: HTMLElement | null) {
  let element = target instanceof Element ? target : null;
  while (element && element !== root) {
    if (element.matches(selector)) return element as HTMLElement;
    element = element.parentElement;
  }
  return null;
}

function siblingIndex(element: HTMLElement, ignoreSelector: string) {
  const parent = element.parentElement;
  if (!parent) return -1;
  return Array.from(parent.children)
    .filter((child) => ignoreSelector === '' || !child.matches(ignoreSelector))
    .indexOf(element);
}

function scrollParent(element: HTMLElement | null) {
  let current = element;
  while (current) {
    const overflow = window.getComputedStyle(current).overflow;
    if (
      (overflow === 'auto' || overflow === 'scroll') &&
      (current.offsetWidth < current.scrollWidth || current.offsetHeight < current.scrollHeight)
    ) {
      return current;
    }
    if (current === document.body) return null;
    current = current.parentElement;
  }
  return null;
}

export function LegacyDragSort({
  children,
  onDragEnd,
  nodeSelector = 'tr',
  handleSelector = '',
  ignoreSelector = '',
  enableScroll = true,
  scrollSpeed = 10,
  lineClassName = '',
}: {
  children: ReactNode;
  onDragEnd: (fromIndex: number, toIndex: number) => void;
  nodeSelector?: string;
  handleSelector?: string;
  ignoreSelector?: string;
  enableScroll?: boolean;
  scrollSpeed?: number;
  lineClassName?: string;
}) {
  const dragList = useRef<HTMLDivElement | null>(null);
  const dragLine = useRef<HTMLDivElement | null>(null);
  const cacheDragTarget = useRef<HTMLElement | null>(null);
  const scrollElement = useRef<HTMLElement | null>(null);
  const scrollTimerId = useRef<number | null>(null);
  const fromIndex = useRef(-1);
  const toIndex = useRef(-1);
  const direction = useRef(3);

  const getDragNode = (target: EventTarget | null) =>
    findClosestWithin(target, nodeSelector, dragList.current);
  const getHandleNode = (target: EventTarget | null) =>
    findClosestWithin(target, handleSelector || nodeSelector, dragList.current);

  const getDragLine = () => {
    if (!dragLine.current) {
      dragLine.current = window.document.createElement('div');
      dragLine.current.setAttribute('style', LEGACY_DRAG_LINE_STYLE);
      window.document.body.appendChild(dragLine.current);
    }
    dragLine.current.className = lineClassName;
    return dragLine.current;
  };

  const hideDragLine = () => {
    if (dragLine.current) dragLine.current.style.display = 'none';
  };

  const fixDragLine = (element: HTMLElement | null) => {
    const line = getDragLine();
    if (!element || fromIndex.current < 0 || fromIndex.current === toIndex.current) {
      hideDragLine();
      return;
    }

    const rect = element.getBoundingClientRect();
    const top = toIndex.current < fromIndex.current ? rect.top : rect.top + rect.height;
    if (enableScroll && scrollElement.current) {
      const scrollRect = scrollElement.current.getBoundingClientRect();
      if (top < scrollRect.top - 2 || top > scrollRect.top + scrollRect.height + 2) {
        hideDragLine();
        return;
      }
    }

    line.style.left = `${rect.left}px`;
    line.style.width = `${rect.width}px`;
    line.style.top = `${top}px`;
    line.style.display = 'block';
  };

  const stopAutoScroll = () => {
    if (scrollTimerId.current !== null) {
      window.clearInterval(scrollTimerId.current);
      scrollTimerId.current = null;
    }
    fixDragLine(cacheDragTarget.current);
  };

  const autoScroll = () => {
    if (!scrollElement.current) return;
    const top = scrollElement.current.scrollTop;
    if (direction.current === 3) {
      scrollElement.current.scrollTop = top + scrollSpeed;
      if (top === scrollElement.current.scrollTop) stopAutoScroll();
    } else if (direction.current === 1) {
      scrollElement.current.scrollTop = top - scrollSpeed;
      if (scrollElement.current.scrollTop <= 0) stopAutoScroll();
    } else {
      stopAutoScroll();
    }
  };

  const resolveAutoScroll = (event: DragEvent, element: HTMLElement) => {
    if (!scrollElement.current) return;
    const rect = scrollElement.current.getBoundingClientRect();
    const zone = element.offsetHeight * (2 / 3);
    direction.current = 0;
    if (event.pageY > rect.top + rect.height - zone) direction.current = 3;
    else if (event.pageY < rect.top + zone) direction.current = 1;
    if (direction.current) {
      if (scrollTimerId.current === null) {
        scrollTimerId.current = window.setInterval(autoScroll, 20);
      }
    } else {
      stopAutoScroll();
    }
  };

  const onDragEnter = (event: DragEvent) => {
    const dragNode = getDragNode(event.target);
    if (dragNode) {
      toIndex.current = siblingIndex(dragNode, ignoreSelector);
      if (enableScroll) resolveAutoScroll(event, dragNode);
    } else {
      toIndex.current = -1;
      stopAutoScroll();
    }
    cacheDragTarget.current = dragNode;
    fixDragLine(dragNode);
  };

  const onDragStart = (event: DragEvent) => {
    const dragNode = getDragNode(event.target);
    if (!dragNode) return;
    const parent = dragNode.parentElement;
    if (!parent) return;
    event.dataTransfer?.setData('Text', '');
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
    parent.ondragenter = onDragEnter;
    parent.ondragover = (dragEvent) => {
      dragEvent.preventDefault();
      return true;
    };
    const index = siblingIndex(dragNode, ignoreSelector);
    fromIndex.current = index;
    toIndex.current = index;
    scrollElement.current = scrollParent(parent);
  };

  const onNativeDragEnd = (event: DragEvent) => {
    const dragNode = getDragNode(event.target);
    stopAutoScroll();
    if (dragNode) {
      dragNode.removeAttribute('draggable');
      dragNode.ondragstart = null;
      dragNode.ondragend = null;
      if (dragNode.parentElement) {
        dragNode.parentElement.ondragenter = null;
        dragNode.parentElement.ondragover = null;
      }
      if (fromIndex.current >= 0 && fromIndex.current !== toIndex.current) {
        onDragEnd(fromIndex.current, toIndex.current);
      }
    }
    hideDragLine();
    fromIndex.current = -1;
    toIndex.current = -1;
  };

  const onMouseDown = (event: ReactMouseEvent<HTMLDivElement>) => {
    const handle = getHandleNode(event.target);
    if (!handle) return;
    const dragNode =
      handleSelector && handleSelector !== nodeSelector ? getDragNode(handle) : handle;
    if (!dragNode) return;
    handle.setAttribute('draggable', 'false');
    dragNode.setAttribute('draggable', 'true');
    dragNode.ondragstart = onDragStart;
    dragNode.ondragend = onNativeDragEnd;
  };

  useEffect(
    () => () => {
      if (dragLine.current?.parentNode) dragLine.current.parentNode.removeChild(dragLine.current);
      dragLine.current = null;
      cacheDragTarget.current = null;
    },
    [],
  );

  return (
    <div role="presentation" onMouseDown={onMouseDown} ref={dragList}>
      {children}
    </div>
  );
}

export function LegacyMenuIcon() {
  return (
    <i aria-label="图标: menu" className="anticon anticon-menu">
      <svg
        viewBox="64 64 896 896"
        focusable="false"
        data-icon="menu"
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M904 160H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8zm0 624H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8zm0-312H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8z" />
      </svg>
    </i>
  );
}
