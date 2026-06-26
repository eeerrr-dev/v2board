import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type HTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { LegacyCloseIcon } from './legacy-ant-icon';
import { LegacyButton, type LegacyButtonProps } from './legacy-button';

type LegacyModalContainer = HTMLElement | false | string | (() => HTMLElement);
type LegacyModalMousePosition = { x: number; y: number };
type LegacyModalButtonProps = Pick<
  LegacyButtonProps,
  'className' | 'disabled' | 'loading' | 'style'
>;

interface LegacyModalProps {
  afterClose?: () => void;
  bodyProps?: HTMLAttributes<HTMLDivElement>;
  bodyStyle?: CSSProperties;
  cancelButtonProps?: LegacyModalButtonProps;
  cancelText?: ReactNode;
  centered?: boolean;
  children?: ReactNode;
  className?: string;
  closable?: boolean;
  closeIcon?: ReactNode;
  confirmLoading?: boolean;
  destroyOnClose?: boolean;
  forceRender?: boolean;
  footer?: ReactNode | boolean | null;
  getContainer?: LegacyModalContainer;
  keyboard?: boolean;
  mask?: boolean;
  maskClosable?: boolean;
  maskProps?: HTMLAttributes<HTMLDivElement>;
  maskStyle?: CSSProperties;
  maskTransitionName?: string;
  mousePosition?: LegacyModalMousePosition;
  okButtonProps?: LegacyModalButtonProps;
  okText?: ReactNode;
  okType?: LegacyButtonProps['type'];
  open?: boolean;
  prefixCls?: string;
  style?: CSSProperties;
  styles?: { body?: CSSProperties };
  title?: ReactNode;
  transitionName?: string;
  visible?: boolean;
  height?: number | string;
  width?: number | string;
  wrapProps?: HTMLAttributes<HTMLDivElement>;
  wrapClassName?: string;
  wrapStyle?: CSSProperties;
  zIndex?: number;
  onCancel?: (event?: MouseEvent<HTMLElement> | ReactKeyboardEvent<HTMLDivElement>) => void;
  onOk?: (event?: MouseEvent<HTMLElement>) => void | Promise<void>;
}

let openModalCount = 0;
let modalTitleIdSeed = 0;

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ');
}

function dimensionalStyle(
  width: number | string | undefined,
  height: number | string | undefined,
): CSSProperties {
  const value = width ?? 520;
  const style: CSSProperties = { width: typeof value === 'number' ? `${value}px` : value };
  if (height !== undefined) style.height = typeof height === 'number' ? `${height}px` : height;
  return style;
}

function zIndexStyle(zIndex: number | undefined): CSSProperties {
  return zIndex === undefined ? {} : { zIndex };
}

function modalStyle(
  width: number | string | undefined,
  height: number | string | undefined,
  style: CSSProperties | undefined,
  transformOrigin: string | undefined,
) {
  return {
    ...style,
    ...dimensionalStyle(width, height),
    ...(transformOrigin ? { transformOrigin } : {}),
  };
}

const FOCUS_SENTINEL_STYLE: CSSProperties = {
  width: 0,
  height: 0,
  overflow: 'hidden',
  outline: 'none',
};

function getPortalContainer(getContainer: LegacyModalContainer | undefined) {
  if (getContainer === false) return false;
  if (typeof document === 'undefined') return undefined;
  if (typeof getContainer === 'string') {
    return document.querySelector<HTMLElement>(getContainer) ?? document.body;
  }
  if (typeof getContainer === 'function') return getContainer();
  return getContainer ?? document.body;
}

export function LegacyModal({
  afterClose,
  bodyProps,
  bodyStyle,
  cancelButtonProps,
  children,
  cancelText = '取消',
  centered,
  className,
  closable = true,
  closeIcon,
  confirmLoading = false,
  destroyOnClose,
  forceRender,
  footer,
  getContainer,
  keyboard = true,
  mask = true,
  maskClosable = true,
  maskProps,
  maskStyle,
  mousePosition,
  okButtonProps,
  okText = '确定',
  okType = 'primary',
  open,
  prefixCls = 'ant-modal',
  style,
  styles,
  title,
  transitionName = 'zoom',
  visible,
  height,
  width,
  wrapProps,
  wrapClassName,
  wrapStyle,
  zIndex,
  onCancel,
  onOk,
}: LegacyModalProps) {
  const isVisible = visible ?? open ?? false;
  const shouldRender = isVisible || forceRender;
  const okLoading = okButtonProps?.loading ?? confirmLoading;
  const mergedBodyStyle = bodyStyle ?? styles?.body;
  const openedAtRef = useRef(0);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const sentinelStartRef = useRef<HTMLDivElement | null>(null);
  const sentinelEndRef = useRef<HTMLDivElement | null>(null);
  const dialogMouseDownRef = useRef(false);
  const wasVisibleRef = useRef(isVisible);
  const titleIdRef = useRef<string | null>(null);
  const [transformOrigin, setTransformOrigin] = useState<string | undefined>();
  const hasTitle = title !== undefined && title !== null;

  if (hasTitle) {
    titleIdRef.current ??= `rcDialogTitle${modalTitleIdSeed++}`;
  }

  useEffect(() => {
    if (!isVisible || typeof document === 'undefined') return;

    openedAtRef.current = Date.now();
    if (openModalCount === 0) document.body.classList.add('ant-modal-open');
    openModalCount += 1;
    wrapRef.current?.focus();

    return () => {
      openModalCount = Math.max(0, openModalCount - 1);
      if (openModalCount === 0) document.body.classList.remove('ant-modal-open');
    };
  }, [isVisible]);

  useEffect(() => {
    if (wasVisibleRef.current && !isVisible) afterClose?.();
    wasVisibleRef.current = isVisible;
  }, [afterClose, isVisible]);

  useEffect(() => {
    if (!isVisible || !mousePosition) {
      setTransformOrigin(undefined);
      return;
    }

    const rect = dialogRef.current?.getBoundingClientRect();
    if (!rect) return;
    setTransformOrigin(`${mousePosition.x - rect.left}px ${mousePosition.y - rect.top}px`);
  }, [isVisible, mousePosition]);

  if (!shouldRender || typeof document === 'undefined') return null;

  const handleCancel = (
    event?: MouseEvent<HTMLElement> | ReactKeyboardEvent<HTMLDivElement>,
  ) => {
    onCancel?.(event);
  };

  const handleMaskClick = (event: MouseEvent<HTMLDivElement>) => {
    if (Date.now() - openedAtRef.current < 300) return;
    if (
      event.target === event.currentTarget &&
      mask &&
      maskClosable &&
      !dialogMouseDownRef.current
    ) {
      handleCancel(event);
    }
  };

  const handleMaskMouseUp = () => {
    if (!dialogMouseDownRef.current) return;
    window.setTimeout(() => {
      dialogMouseDownRef.current = false;
    }, 0);
  };

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    if (keyboard && (event.key === 'Escape' || event.keyCode === 27)) {
      event.stopPropagation();
      handleCancel(event);
      return;
    }

    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    if (event.key !== 'Tab' && event.keyCode !== 9) return;
    if (event.shiftKey && document.activeElement === sentinelStartRef.current) {
      event.preventDefault();
      sentinelEndRef.current?.focus();
    } else if (!event.shiftKey && document.activeElement === sentinelEndRef.current) {
      event.preventDefault();
      sentinelStartRef.current?.focus();
    }
  };

  const root = (
    <div className={`${prefixCls}-root`}>
      {mask ? (
        <div
          {...maskProps}
          className={classNames(
            `${prefixCls}-mask`,
            !isVisible && `${prefixCls}-mask-hidden`,
            maskProps?.className,
          )}
          style={{ ...zIndexStyle(zIndex), ...maskStyle, ...maskProps?.style }}
        />
      ) : null}
      <div
        {...wrapProps}
        ref={wrapRef}
        tabIndex={-1}
        className={classNames(
          `${prefixCls}-wrap`,
          centered && `${prefixCls}-centered`,
          wrapClassName,
          wrapProps?.className,
        )}
        role="dialog"
        aria-labelledby={hasTitle ? (titleIdRef.current ?? undefined) : undefined}
        style={{
          ...zIndexStyle(zIndex),
          ...wrapStyle,
          ...wrapProps?.style,
          display: isVisible ? undefined : 'none',
        }}
        onClick={handleMaskClick}
        onMouseUp={mask && maskClosable ? handleMaskMouseUp : undefined}
        onKeyDown={handleKeyDown}
      >
        <div
          ref={dialogRef}
          className={classNames(
            prefixCls,
            isVisible && transitionName && `${transitionName}-appear`,
            isVisible && transitionName && `${transitionName}-appear-active`,
            className,
          )}
          role="document"
          style={modalStyle(width, height, style, transformOrigin)}
          onMouseDown={() => {
            dialogMouseDownRef.current = true;
          }}
        >
          <div
            ref={sentinelStartRef}
            tabIndex={0}
            style={FOCUS_SENTINEL_STYLE}
            aria-hidden="true"
          />
          <div className={`${prefixCls}-content`}>
            {closable ? (
              <button
                type="button"
                aria-label="Close"
                className={`${prefixCls}-close`}
                onClick={handleCancel}
              >
                <span className={`${prefixCls}-close-x`}>
                  {closeIcon ?? <LegacyCloseIcon className={`${prefixCls}-close-icon`} />}
                </span>
              </button>
            ) : null}
            {hasTitle ? (
              <div className={`${prefixCls}-header`}>
                <div className={`${prefixCls}-title`} id={titleIdRef.current ?? undefined}>
                  {title}
                </div>
              </div>
            ) : null}
            {destroyOnClose && !isVisible ? null : (
              <>
                <div
                  {...bodyProps}
                  className={classNames(`${prefixCls}-body`, bodyProps?.className)}
                  style={{ ...mergedBodyStyle, ...bodyProps?.style }}
                >
                  {children}
                </div>
                {footer === false || footer === null ? null : (
                  <div className={`${prefixCls}-footer`}>
                    {footer ?? (
                      <div>
                        <LegacyButton
                          {...cancelButtonProps}
                          className={classNames('ant-btn', cancelButtonProps?.className)}
                          onClick={handleCancel}
                        >
                          {cancelText}
                        </LegacyButton>
                        <LegacyButton
                          {...okButtonProps}
                          className={classNames('ant-btn', okButtonProps?.className)}
                          loading={okLoading}
                          type={okType}
                          onClick={onOk}
                        >
                          {okText}
                        </LegacyButton>
                      </div>
                    )}
                  </div>
                )}
              </>
            )}
          </div>
          <div
            ref={sentinelEndRef}
            tabIndex={0}
            style={FOCUS_SENTINEL_STYLE}
            aria-hidden="true"
          />
        </div>
      </div>
    </div>
  );

  const portalContainer = getPortalContainer(getContainer);
  return portalContainer === false ? root : createPortal(root, portalContainer ?? document.body);
}
