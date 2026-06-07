import {
  cloneElement,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
  type ReactElement,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';

export type LegacyDropdownTrigger = 'click' | 'hover';

type LegacyDropdownChildProps = {
  className?: string;
  onClick?: (event: ReactMouseEvent<HTMLElement>) => void;
  onMouseEnter?: (event: ReactMouseEvent<HTMLElement>) => void;
  onMouseLeave?: (event: ReactMouseEvent<HTMLElement>) => void;
};

export interface LegacyDropdownProps {
  children: ReactElement<LegacyDropdownChildProps>;
  disabled?: boolean;
  overlay: ReactNode;
  trigger?: LegacyDropdownTrigger | LegacyDropdownTrigger[];
}

interface LegacyDropdownCoords {
  left: number;
  top: number;
  minWidth: number;
}

export const LEGACY_DROPDOWN_CLICK_TRIGGER = 'click' satisfies LegacyDropdownTrigger;

const LEGACY_DROPDOWN_HOVER_CLOSE_DELAY = 120;
const LEGACY_DROPDOWN_OFFSET = 4;

function dropdownTriggerModes(trigger: LegacyDropdownProps['trigger']) {
  return Array.isArray(trigger) ? trigger : trigger ? [trigger] : ['hover'];
}

function legacyDropdownClassName(open: boolean) {
  return [
    'ant-dropdown',
    'ant-dropdown-placement-bottomLeft',
    open ? undefined : 'ant-dropdown-hidden',
  ]
    .filter(Boolean)
    .join(' ');
}

function mergeClassName(...values: Array<string | undefined | false>) {
  return values.filter(Boolean).join(' ');
}

export function LegacyDropdown({ children, disabled, overlay, trigger }: LegacyDropdownProps) {
  const [open, setOpen] = useState(false);
  const [hasOpened, setHasOpened] = useState(false);
  const [coords, setCoords] = useState<LegacyDropdownCoords>();
  const popupRef = useRef<HTMLDivElement | null>(null);
  const closeTimer = useRef<number | undefined>(undefined);
  const triggerModes = dropdownTriggerModes(trigger);
  const opensOnClick = triggerModes.includes('click');
  const opensOnHover = triggerModes.includes('hover');

  const clearCloseTimer = () => {
    if (closeTimer.current !== undefined) {
      window.clearTimeout(closeTimer.current);
      closeTimer.current = undefined;
    }
  };

  const openFromElement = (element: HTMLElement) => {
    const rect = element.getBoundingClientRect();
    clearCloseTimer();
    setCoords({
      left: rect.left,
      top: rect.bottom + LEGACY_DROPDOWN_OFFSET,
      minWidth: rect.width,
    });
    setHasOpened(true);
    setOpen(true);
  };

  const scheduleHoverClose = () => {
    if (!opensOnHover) return;
    clearCloseTimer();
    closeTimer.current = window.setTimeout(() => setOpen(false), LEGACY_DROPDOWN_HOVER_CLOSE_DELAY);
  };

  useEffect(() => {
    if (!open || !opensOnClick) return undefined;
    const closeOnOutsideClick = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target) return;
      if (popupRef.current?.contains(target)) return;
      if (target.closest('.ant-dropdown-trigger')) return;
      setOpen(false);
    };

    document.addEventListener('click', closeOnOutsideClick);
    return () => document.removeEventListener('click', closeOnOutsideClick);
  }, [open, opensOnClick]);

  useEffect(() => {
    return () => clearCloseTimer();
  }, []);

  const triggerElement = cloneElement(children, {
    className: mergeClassName(
      children.props.className,
      !disabled && 'ant-dropdown-trigger',
      !disabled && open && 'ant-dropdown-open',
    ),
    onClick: (event: ReactMouseEvent<HTMLElement>) => {
      children.props.onClick?.(event);
      if (disabled) return;
      if (opensOnClick) {
        if (open) {
          setOpen(false);
        } else {
          openFromElement(event.currentTarget);
        }
        return;
      }
      openFromElement(event.currentTarget);
    },
    onMouseEnter: (event: ReactMouseEvent<HTMLElement>) => {
      children.props.onMouseEnter?.(event);
      if (disabled) return;
      if (opensOnHover) openFromElement(event.currentTarget);
    },
    onMouseLeave: (event: ReactMouseEvent<HTMLElement>) => {
      children.props.onMouseLeave?.(event);
      if (disabled) return;
      scheduleHoverClose();
    },
  });

  return (
    <>
      {triggerElement}
      {hasOpened && coords && typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={popupRef}
              className={legacyDropdownClassName(open)}
              style={{
                position: 'fixed',
                top: coords.top,
                left: coords.left,
                minWidth: coords.minWidth,
              }}
              onClick={() => setOpen(false)}
              onMouseEnter={clearCloseTimer}
              onMouseLeave={scheduleHoverClose}
            >
              {overlay}
            </div>,
            document.body,
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
  onClick,
  onContextMenu,
  style,
}: {
  children?: ReactNode;
  disabled?: boolean;
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
