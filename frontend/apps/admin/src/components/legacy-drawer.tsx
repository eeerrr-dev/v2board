import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { LegacyCloseIcon } from './legacy-ant-icon';

type LegacyDrawerPlacement = 'top' | 'right' | 'bottom' | 'left';
type LegacyDrawerContainer = string | HTMLElement | false | (() => HTMLElement);

interface LegacyDrawerContextValue {
  pull: () => void;
  push: () => void;
}

interface LegacyDrawerProps {
  afterVisibleChange?: (visible: boolean) => void;
  bodyStyle?: CSSProperties;
  cancelText?: string;
  children?: ReactNode;
  className?: string;
  closable?: boolean;
  destroyOnClose?: boolean;
  drawerStyle?: CSSProperties;
  forceRender?: boolean;
  footer?: ReactNode;
  getContainer?: LegacyDrawerContainer;
  headerStyle?: CSSProperties;
  height?: number | string;
  id?: string;
  keyboard?: boolean;
  mask?: boolean;
  maskClosable?: boolean;
  maskStyle?: CSSProperties;
  open?: boolean;
  placement?: LegacyDrawerPlacement;
  prefixCls?: string;
  style?: CSSProperties;
  title?: ReactNode;
  visible?: boolean;
  width?: number | string;
  wrapClassName?: string;
  zIndex?: number;
  onClose?: (event?: MouseEvent<HTMLElement> | ReactKeyboardEvent<HTMLDivElement>) => void;
}

let openDrawerCount = 0;
let previousBodyOverflow = '';
let previousBodyTouchAction = '';
const LegacyDrawerContext = createContext<LegacyDrawerContextValue | null>(null);

function classNames(...values: Array<string | false | undefined>) {
  return values.filter(Boolean).join(' ') || undefined;
}

function sizeValue(value: number | string | undefined) {
  if (typeof value === 'number') return `${value}px`;
  return value;
}

function contentWrapperStyle(
  placement: LegacyDrawerPlacement,
  width: number | string,
  height: number | string,
  open: boolean,
): CSSProperties {
  const horizontal = placement === 'left' || placement === 'right';
  const translate = horizontal ? 'translateX' : 'translateY';
  const hiddenOffset = placement === 'left' || placement === 'top' ? '-100%' : '100%';
  const transform = open ? undefined : `${translate}(${hiddenOffset})`;
  return {
    transform,
    msTransform: transform,
    width: horizontal ? sizeValue(width) : undefined,
    height: horizontal ? undefined : sizeValue(height),
  };
}

function drawerRootClassName({
  className,
  mask,
  open,
  placement,
  prefixCls,
  wrapClassName,
}: {
  className: string | undefined;
  mask: boolean;
  open: boolean;
  placement: LegacyDrawerPlacement;
  prefixCls: string;
  wrapClassName?: string;
}) {
  return classNames(
    prefixCls,
    `${prefixCls}-${placement}`,
    open && `${prefixCls}-open`,
    wrapClassName,
    className,
    !mask && 'no-mask',
  );
}

function getPortalContainer(getContainer: LegacyDrawerContainer | undefined) {
  if (typeof document === 'undefined') return null;
  if (getContainer === false) return false;
  if (typeof getContainer === 'string') return document.querySelector<HTMLElement>(getContainer);
  if (typeof getContainer === 'function') return getContainer();
  return getContainer ?? document.body;
}

function getPushTransform(placement: LegacyDrawerPlacement) {
  if (placement === 'left' || placement === 'right') {
    return `translateX(${placement === 'left' ? 180 : -180}px)`;
  }
  if (placement === 'top' || placement === 'bottom') {
    return `translateY(${placement === 'top' ? 180 : -180}px)`;
  }
  return undefined;
}

export function LegacyDrawer({
  afterVisibleChange,
  bodyStyle,
  cancelText,
  children,
  className,
  closable = true,
  destroyOnClose,
  drawerStyle,
  forceRender,
  footer,
  getContainer,
  headerStyle,
  height = 256,
  id,
  keyboard = true,
  mask = true,
  maskClosable = true,
  maskStyle,
  open,
  placement = 'right',
  prefixCls = 'ant-drawer',
  style,
  title,
  visible,
  width,
  wrapClassName,
  zIndex,
  onClose,
}: LegacyDrawerProps) {
  const parentDrawer = useContext(LegacyDrawerContext);
  const drawerRef = useRef<HTMLDivElement | null>(null);
  const [pushed, setPushed] = useState(false);
  const isVisible = visible ?? open ?? false;
  const shouldRender = isVisible || forceRender;

  const push = useCallback(() => setPushed(true), []);
  const pull = useCallback(() => setPushed(false), []);
  const contextValue = useMemo(() => ({ pull, push }), [pull, push]);

  useEffect(() => {
    if (!isVisible || typeof document === 'undefined') return;
    drawerRef.current?.focus();
  }, [isVisible]);

  useEffect(() => {
    if (!parentDrawer) return;
    if (isVisible) {
      parentDrawer.push();
      return () => parentDrawer.pull();
    }
    parentDrawer.pull();
    return undefined;
  }, [isVisible, parentDrawer]);

  useEffect(() => {
    if (!isVisible || !mask || typeof document === 'undefined') return;
    if (openDrawerCount === 0) {
      previousBodyOverflow = document.body.style.overflow;
      previousBodyTouchAction = document.body.style.touchAction;
      document.body.style.overflow = 'hidden';
    }
    openDrawerCount += 1;
    document.body.style.touchAction = 'none';

    return () => {
      openDrawerCount = Math.max(0, openDrawerCount - 1);
      if (openDrawerCount === 0) {
        document.body.style.overflow = previousBodyOverflow;
        document.body.style.touchAction = previousBodyTouchAction;
      }
    };
  }, [isVisible, mask]);

  useEffect(() => {
    if (!shouldRender) return;
    afterVisibleChange?.(isVisible);
  }, [afterVisibleChange, isVisible, shouldRender]);

  if (!shouldRender || typeof document === 'undefined') return null;

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (isVisible && keyboard && (event.key === 'Escape' || event.keyCode === 27)) {
      event.stopPropagation();
      onClose?.(event);
    }
  };

  const header =
    title || closable ? (
      <div
        className={`${prefixCls}${title ? '-header' : '-header-no-title'}`}
        style={headerStyle}
      >
        {title ? <div className={`${prefixCls}-title`}>{title}</div> : null}
        {closable ? (
          <button
            aria-label="Close"
            className={`${prefixCls}-close`}
            onClick={(event) => onClose?.(event)}
          >
            <LegacyCloseIcon />
          </button>
        ) : null}
      </div>
    ) : null;

  const drawer = (
    <div
      ref={drawerRef}
      id={id}
      {...(cancelText !== undefined ? { canceltext: cancelText } : {})}
      {...(footer !== undefined ? { footer: String(footer) } : {})}
      tabIndex={-1}
      className={drawerRootClassName({
        className,
        mask,
        open: isVisible,
        placement,
        prefixCls,
        wrapClassName,
      })}
      style={{ zIndex, transform: pushed ? getPushTransform(placement) : undefined, ...style }}
      onKeyDown={handleKeyDown}
    >
      {mask ? (
        <div
          className={`${prefixCls}-mask`}
          onClick={maskClosable ? (event) => onClose?.(event) : undefined}
          style={maskStyle}
        />
      ) : null}
      <div
        className={`${prefixCls}-content-wrapper`}
        style={contentWrapperStyle(placement, width ?? 256, height, isVisible)}
      >
        <div className={`${prefixCls}-content`}>
          {destroyOnClose && !isVisible ? null : (
            <div className={`${prefixCls}-wrapper-body`} style={drawerStyle}>
              {header}
              <div className={`${prefixCls}-body`} style={bodyStyle}>
                {children}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );

  const wrappedDrawer = (
    <LegacyDrawerContext.Provider value={contextValue}>{drawer}</LegacyDrawerContext.Provider>
  );

  const portalContainer = getPortalContainer(getContainer);
  if (portalContainer === false) return wrappedDrawer;
  if (!portalContainer) return null;
  return createPortal(wrappedDrawer, portalContainer);
}
