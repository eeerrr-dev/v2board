import {
  cloneElement,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type MutableRefObject,
  type MouseEvent as ReactMouseEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
  type RefAttributes,
} from 'react';
import { createPortal } from 'react-dom';

export type LegacyDropdownTrigger = 'click' | 'hover' | 'contextMenu';
type LegacyDropdownPlacement =
  | 'bottomLeft'
  | 'bottomCenter'
  | 'bottomRight'
  | 'topLeft'
  | 'topCenter'
  | 'topRight';

type LegacyDropdownChildProps = {
  className?: string;
  disabled?: boolean;
  onClick?: (event: ReactMouseEvent<HTMLElement>) => void;
  onContextMenu?: (event: ReactMouseEvent<HTMLElement>) => void;
  onMouseEnter?: (event: ReactMouseEvent<HTMLElement>) => void;
  onMouseLeave?: (event: ReactMouseEvent<HTMLElement>) => void;
};

export interface LegacyDropdownProps {
  children: ReactElement<LegacyDropdownChildProps>;
  defaultVisible?: boolean;
  disabled?: boolean;
  getPopupContainer?: (triggerNode: HTMLElement) => HTMLElement | null;
  onOverlayClick?: (event: ReactMouseEvent<HTMLDivElement>) => void;
  onVisibleChange?: (visible: boolean) => void;
  overlay: ReactNode | (() => ReactNode);
  overlayClassName?: string;
  overlayStyle?: CSSProperties;
  placement?: LegacyDropdownPlacement;
  trigger?: LegacyDropdownTrigger | LegacyDropdownTrigger[];
  visible?: boolean;
  mouseEnterDelay?: number;
  mouseLeaveDelay?: number;
  openClassName?: string;
}

interface LegacyDropdownCoords {
  container: HTMLElement;
  placement: LegacyDropdownPlacement;
  left: number;
  top: number;
  minWidth: number;
  source:
    | {
        type: 'trigger';
        bottom: number;
        height: number;
        left: number;
        right: number;
        top: number;
        width: number;
      }
    | { type: 'point'; pageX: number; pageY: number };
}

export const LEGACY_DROPDOWN_CLICK_TRIGGER = 'click' satisfies LegacyDropdownTrigger;

const LEGACY_DROPDOWN_OFFSET = 4;

function dropdownTriggerModes(trigger: LegacyDropdownProps['trigger']) {
  return Array.isArray(trigger) ? trigger : trigger ? [trigger] : ['hover'];
}

function legacyDropdownClassName(
  open: boolean,
  placement: NonNullable<LegacyDropdownProps['placement']>,
  overlayClassName?: string,
) {
  // rc-trigger joins `prefixCls + " " + popupClassName + " " + placementCls` (double space when
  // rc-dropdown's default overlayClassName "" fills the middle slot) and appends "-hidden" last.
  const base = `ant-dropdown ${overlayClassName ?? ''} ant-dropdown-placement-${placement}`;
  return open ? base : `${base} ant-dropdown-hidden`;
}

function mergeClassName(...values: Array<string | undefined | false>) {
  return values.filter(Boolean).join(' ');
}

function secondsToMs(value: number) {
  return value * 1000;
}

function pointInsideElement(element: HTMLElement, clientX: number, clientY: number) {
  const rect = element.getBoundingClientRect();
  return (
    clientX >= rect.left &&
    clientX <= rect.right &&
    clientY >= rect.top &&
    clientY <= rect.bottom
  );
}

function triggerSourceFromElement(element: HTMLElement): LegacyDropdownCoords['source'] {
  const rect = element.getBoundingClientRect();
  return {
    type: 'trigger',
    bottom: rect.bottom + window.scrollY,
    height: rect.height,
    left: rect.left + window.scrollX,
    right: rect.right + window.scrollX,
    top: rect.top + window.scrollY,
    width: rect.width,
  };
}

function getAlignedCoords(
  source: LegacyDropdownCoords['source'],
  placement: LegacyDropdownPlacement,
  popup?: HTMLElement | null,
) {
  const popupWidth = popup?.offsetWidth ?? (source.type === 'trigger' ? source.width : 0);
  const popupHeight = popup?.offsetHeight ?? 0;
  const horizontalPlacement = placement.replace(/^top|^bottom/, '');
  const verticalPlacement = placement.startsWith('top') ? 'top' : 'bottom';
  const targetLeft = source.type === 'trigger' ? source.left : source.pageX;
  const targetRight = source.type === 'trigger' ? source.right : source.pageX;
  const targetCenter =
    source.type === 'trigger' ? source.left + source.width / 2 : source.pageX;
  const targetTop = source.type === 'trigger' ? source.top : source.pageY;
  const targetBottom = source.type === 'trigger' ? source.bottom : source.pageY;
  const left =
    horizontalPlacement === 'Right'
      ? targetRight - popupWidth
      : horizontalPlacement === 'Center'
        ? targetCenter - popupWidth / 2
        : targetLeft;
  const top =
    verticalPlacement === 'top'
      ? targetTop - popupHeight - LEGACY_DROPDOWN_OFFSET
      : targetBottom + LEGACY_DROPDOWN_OFFSET;

  return { left, top };
}

export function LegacyDropdown(props: LegacyDropdownProps) {
  const {
    children,
    defaultVisible = false,
    disabled,
    getPopupContainer,
    onOverlayClick,
    onVisibleChange,
    overlay,
    overlayClassName,
    overlayStyle,
    placement = 'bottomLeft',
    trigger,
    visible,
    mouseEnterDelay = 0.15,
    mouseLeaveDelay = 0.1,
    openClassName,
  } = props;
  const isControlled = Object.prototype.hasOwnProperty.call(props, 'visible');
  const [open, setOpen] = useState(defaultVisible);
  const [hasOpened, setHasOpened] = useState(defaultVisible || !!visible);
  const [overlayPinned, setOverlayPinned] = useState(false);
  const [hoverSuppressed, setHoverSuppressed] = useState(false);
  const [coords, setCoords] = useState<LegacyDropdownCoords>();
  const triggerRef = useRef<HTMLElement | null>(null);
  const popupRef = useRef<HTMLDivElement | null>(null);
  const delayTimer = useRef<number | undefined>(undefined);
  const triggerModes = dropdownTriggerModes(trigger);
  const opensOnClick = triggerModes.includes('click');
  const opensOnHover = triggerModes.includes('hover');
  const opensOnContextMenu = triggerModes.includes('contextMenu');
  const actualOpen = isControlled ? !!visible : open;

  const clearDelayTimer = () => {
    if (delayTimer.current !== undefined) {
      window.clearTimeout(delayTimer.current);
      delayTimer.current = undefined;
    }
  };

  const setVisibleState = (nextOpen: boolean) => {
    if (actualOpen === nextOpen) return;
    if (!isControlled) setOpen(nextOpen);
    onVisibleChange?.(nextOpen);
  };

  const closeFromOverlayClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest('.ant-dropdown-menu-item-disabled')) return;
    if (target?.closest('[data-legacy-dropdown-keep-open="true"]')) {
      onOverlayClick?.(event);
      return;
    }
    clearDelayTimer();
    onOverlayClick?.(event);
    if (opensOnClick || opensOnContextMenu) {
      setOverlayPinned(false);
      setVisibleState(false);
      return;
    }
    if (opensOnHover) setHoverSuppressed(true);
    setOverlayPinned(false);
    setVisibleState(false);
  };

  const setCoordsFromSource = (
    element: HTMLElement,
    source: LegacyDropdownCoords['source'],
    nextPlacement = placement,
  ) => {
    const aligned = getAlignedCoords(source, nextPlacement, popupRef.current);
    setCoords({
      container: getPopupContainer?.(element) ?? document.body,
      placement: nextPlacement,
      minWidth: source.type === 'trigger' ? source.width : 0,
      source,
      ...aligned,
    });
  };

  const openFromSource = (element: HTMLElement, source: LegacyDropdownCoords['source']) => {
    clearDelayTimer();
    setCoordsFromSource(element, source);
    setHasOpened(true);
    setVisibleState(true);
  };

  const openFromElement = (element: HTMLElement) => {
    openFromSource(element, triggerSourceFromElement(element));
  };

  const scheduleVisibleState = (
    nextOpen: boolean,
    delaySeconds: number,
    sourceElement?: HTMLElement,
  ) => {
    clearDelayTimer();
    const delay = secondsToMs(delaySeconds);
    if (!delay) {
      if (nextOpen && sourceElement) {
        openFromElement(sourceElement);
      } else {
        setVisibleState(nextOpen);
      }
      return;
    }
    delayTimer.current = window.setTimeout(() => {
      if (nextOpen && sourceElement) {
        openFromElement(sourceElement);
      } else {
        setVisibleState(nextOpen);
      }
      clearDelayTimer();
    }, delay);
  };

  useEffect(() => {
    if (!actualOpen || (!opensOnClick && !opensOnContextMenu)) return undefined;
    const closeOnOutsidePointer = (event: MouseEvent | TouchEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target) return;
      if (popupRef.current?.contains(target)) return;
      if (triggerRef.current?.contains(target)) return;
      if (
        overlayPinned &&
        target.closest('.ant-modal, .ant-drawer') &&
        !target.closest(
          '.ant-modal-footer, .ant-modal-confirm-btns, .ant-modal-close, .v2board-drawer-action, .ant-drawer-close',
        )
      ) {
        return;
      }
      setOverlayPinned(false);
      setVisibleState(false);
    };

    document.addEventListener('mousedown', closeOnOutsidePointer);
    document.addEventListener('touchstart', closeOnOutsidePointer);
    return () => {
      document.removeEventListener('mousedown', closeOnOutsidePointer);
      document.removeEventListener('touchstart', closeOnOutsidePointer);
    };
  }, [actualOpen, opensOnClick, opensOnContextMenu, overlayPinned]);

  useEffect(() => {
    if (!actualOpen || !overlayPinned || opensOnClick || opensOnContextMenu) return undefined;
    const closePinnedHoverOverlay = (event: MouseEvent | TouchEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target) return;
      if (popupRef.current?.contains(target)) return;
      if (triggerRef.current?.contains(target)) return;
      if (
        target.closest('.ant-modal') &&
        !target.closest('.ant-modal-footer') &&
        !target.closest('.ant-modal-confirm-btns') &&
        !target.closest('.ant-modal-close')
      ) {
        return;
      }
      setOverlayPinned(false);
      setVisibleState(false);
    };

    document.addEventListener('mousedown', closePinnedHoverOverlay, true);
    document.addEventListener('click', closePinnedHoverOverlay, true);
    document.addEventListener('touchstart', closePinnedHoverOverlay, true);
    return () => {
      document.removeEventListener('mousedown', closePinnedHoverOverlay, true);
      document.removeEventListener('click', closePinnedHoverOverlay, true);
      document.removeEventListener('touchstart', closePinnedHoverOverlay, true);
    };
  }, [actualOpen, opensOnClick, opensOnContextMenu, overlayPinned]);

  useEffect(() => {
    if (!hoverSuppressed) return undefined;
    const releaseHoverSuppression = (event: MouseEvent | TouchEvent) => {
      const triggerNode = triggerRef.current;
      if (!triggerNode) {
        setHoverSuppressed(false);
        return;
      }
      const points =
        'touches' in event
          ? Array.from(event.touches)
          : [{ clientX: event.clientX, clientY: event.clientY }];
      if (points.every((point) => !pointInsideElement(triggerNode, point.clientX, point.clientY))) {
        setHoverSuppressed(false);
      }
    };

    document.addEventListener('mousemove', releaseHoverSuppression, true);
    document.addEventListener('touchstart', releaseHoverSuppression, true);
    return () => {
      document.removeEventListener('mousemove', releaseHoverSuppression, true);
      document.removeEventListener('touchstart', releaseHoverSuppression, true);
    };
  }, [hoverSuppressed]);

  useEffect(() => {
    return () => clearDelayTimer();
  }, []);

  useLayoutEffect(() => {
    if (!actualOpen) return;
    const triggerNode = triggerRef.current;
    if (!triggerNode) return;
    if (!coords) {
      setCoordsFromSource(triggerNode, triggerSourceFromElement(triggerNode));
      setHasOpened(true);
      return;
    }
    const aligned = getAlignedCoords(coords.source, coords.placement, popupRef.current);
    if (aligned.left !== coords.left || aligned.top !== coords.top) {
      setCoords((current) => (current ? { ...current, ...aligned } : current));
    }
  }, [actualOpen, coords, placement]);

  const childRef =
    'ref' in children
      ? (children as ReactElement<LegacyDropdownChildProps> & {
          ref?: Ref<HTMLElement>;
        }).ref
      : undefined;

  const triggerElement = cloneElement(
    children as ReactElement<LegacyDropdownChildProps & RefAttributes<HTMLElement>>,
    {
      ref: (node: HTMLElement | null) => {
        triggerRef.current = node;
        if (typeof childRef === 'function') {
          childRef(node);
        } else if (childRef && typeof childRef === 'object') {
          (childRef as MutableRefObject<HTMLElement | null>).current = node;
        }
      },
      className: mergeClassName(
        children.props.className,
        'ant-dropdown-trigger',
        !disabled && actualOpen && (openClassName ?? 'ant-dropdown-open'),
      ),
      disabled,
      onClick: (event: ReactMouseEvent<HTMLElement>) => {
        children.props.onClick?.(event);
        if (disabled || !opensOnClick) return;
        event.preventDefault();
        if (actualOpen) {
          setVisibleState(false);
        } else {
          openFromElement(event.currentTarget);
        }
      },
      onContextMenu: (event: ReactMouseEvent<HTMLElement>) => {
        children.props.onContextMenu?.(event);
        if (disabled || !opensOnContextMenu) return;
        event.preventDefault();
        openFromSource(event.currentTarget, {
          type: 'point',
          pageX: event.pageX || event.clientX + window.scrollX,
          pageY: event.pageY || event.clientY + window.scrollY,
        });
      },
      onMouseEnter: (event: ReactMouseEvent<HTMLElement>) => {
        children.props.onMouseEnter?.(event);
        if (disabled) return;
        if (hoverSuppressed) return;
        if (opensOnHover) scheduleVisibleState(true, mouseEnterDelay, event.currentTarget);
      },
      onMouseLeave: (event: ReactMouseEvent<HTMLElement>) => {
        children.props.onMouseLeave?.(event);
        if (disabled) return;
        if (opensOnHover) scheduleVisibleState(false, mouseLeaveDelay);
      },
    },
  );

  const actualOverlay =
    hasOpened && coords ? (typeof overlay === 'function' ? overlay() : overlay) : null;

  return (
    <>
      {triggerElement}
      {hasOpened && coords && typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={popupRef}
              className={legacyDropdownClassName(actualOpen, placement, overlayClassName)}
              style={{
                position: 'absolute',
                top: coords.top,
                left: coords.left,
                minWidth: coords.minWidth,
                ...overlayStyle,
              }}
              onClickCapture={closeFromOverlayClick}
              onMouseEnter={clearDelayTimer}
              onMouseLeave={() => {
                if (opensOnHover && !overlayPinned) scheduleVisibleState(false, mouseLeaveDelay);
              }}
            >
              {actualOverlay}
            </div>,
            coords.container,
          )
        : null}
    </>
  );
}

export function LegacyDropdownMenu({ children }: { children: ReactNode }) {
  return (
    <ul className="ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical">
      {children}
    </ul>
  );
}

export function LegacyDropdownMenuItem({
  children,
  disabled,
  keepOpenOnClick,
  onClick,
  onContextMenu,
  style,
}: {
  children?: ReactNode;
  disabled?: boolean;
  keepOpenOnClick?: boolean;
  onClick?: (event: ReactMouseEvent<HTMLLIElement>) => void;
  onContextMenu?: (event: ReactMouseEvent<HTMLLIElement>) => void;
  style?: CSSProperties;
}) {
  return (
    <li
      className={mergeClassName(
        'ant-dropdown-menu-item',
        disabled && 'ant-dropdown-menu-item-disabled',
      )}
      role="menuitem"
      aria-disabled={disabled || undefined}
      data-legacy-dropdown-keep-open={keepOpenOnClick || undefined}
      style={style}
      onClickCapture={(event) => {
        if (!disabled) return;
        event.preventDefault();
        event.stopPropagation();
      }}
      onClick={disabled ? undefined : onClick}
      onContextMenu={onContextMenu}
    >
      {children}
    </li>
  );
}
