import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import {
  LegacyCheckCircleIcon,
  LegacyCloseCircleIcon,
  LegacyExclamationCircleIcon,
  LegacyInfoCircleIcon,
  LegacyQuestionCircleIcon,
} from './legacy-ant-icon';
import { LegacyButton, type LegacyButtonProps } from './legacy-button';

interface LegacyConfirmOptions {
  autoFocusButton?: 'ok' | 'cancel' | null;
  cancelButtonProps?: LegacyConfirmButtonProps;
  cancelText?: ReactNode;
  centered?: boolean;
  className?: string;
  content?: ReactNode;
  getContainer?: LegacyConfirmContainer;
  icon?: ReactNode;
  iconType?: LegacyConfirmIconType;
  keyboard?: boolean;
  mask?: boolean;
  maskClosable?: boolean;
  maskStyle?: CSSProperties;
  okButtonProps?: LegacyConfirmButtonProps;
  okCancel?: boolean;
  okText?: ReactNode;
  okType?: LegacyButtonProps['type'];
  prefixCls?: string;
  showCancel?: boolean;
  style?: CSSProperties;
  title?: ReactNode;
  type?: LegacyConfirmType;
  width?: number | string;
  zIndex?: number;
  onCancel?: LegacyConfirmAction;
  onOk?: LegacyConfirmAction;
}

type LegacyConfirmAction = (...args: unknown[]) => unknown;
type LegacyConfirmButtonProps = Pick<
  LegacyButtonProps,
  'className' | 'disabled' | 'loading' | 'style'
>;
type LegacyConfirmContainer = HTMLElement | false | string | (() => HTMLElement);
type LegacyConfirmIconType =
  | 'check-circle'
  | 'close-circle'
  | 'exclamation-circle'
  | 'info-circle'
  | 'question-circle';
type LegacyConfirmType = 'confirm' | 'error' | 'info' | 'success' | 'warning';
type LegacyConfirmHandle = Promise<boolean> & {
  destroy: () => void;
  update: (options: Partial<LegacyConfirmOptions>) => void;
};

interface LegacyConfirmRequest {
  id: number;
  options: LegacyConfirmOptions;
  resolve: (value: boolean) => void;
  settled?: boolean;
}

let nextId = 1;
let queue: LegacyConfirmRequest[] = [];
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((listener) => listener());
}

function resolveRequest(request: LegacyConfirmRequest, value: boolean) {
  if (request.settled) return;
  request.settled = true;
  request.resolve(value);
}

function destroyRequest(id: number) {
  const request = queue.find((item) => item.id === id);
  if (!request) return;
  resolveRequest(request, false);
  queue = queue.filter((item) => item.id !== id);
  emit();
}

export function legacyConfirm(options: LegacyConfirmOptions): LegacyConfirmHandle {
  let request!: LegacyConfirmRequest;
  const promise = new Promise<boolean>((resolve) => {
    request = { id: nextId++, options: { type: 'confirm', ...options }, resolve };
    queue = [...queue, request];
    emit();
  }) as LegacyConfirmHandle;

  promise.destroy = () => destroyRequest(request.id);
  promise.update = (nextOptions) => {
    queue = queue.map((item) =>
      item.id === request.id ? { ...item, options: { ...item.options, ...nextOptions } } : item,
    );
    emit();
  };

  return promise;
}

export function legacyInfo(options: LegacyConfirmOptions): LegacyConfirmHandle {
  return legacyConfirm({
    type: 'info',
    icon: <LegacyInfoCircleIcon />,
    okCancel: false,
    ...options,
  });
}

export function legacySuccess(options: LegacyConfirmOptions): LegacyConfirmHandle {
  return legacyConfirm({
    type: 'success',
    icon: <LegacyCheckCircleIcon />,
    okCancel: false,
    ...options,
  });
}

export function legacyError(options: LegacyConfirmOptions): LegacyConfirmHandle {
  return legacyConfirm({
    type: 'error',
    icon: <LegacyCloseCircleIcon />,
    okCancel: false,
    ...options,
  });
}

export function legacyWarning(options: LegacyConfirmOptions): LegacyConfirmHandle {
  return legacyConfirm({
    type: 'warning',
    icon: <LegacyExclamationCircleIcon />,
    okCancel: false,
    ...options,
  });
}

export const legacyWarn = legacyWarning;

export function legacyDestroyAll() {
  queue.forEach((request) => resolveRequest(request, false));
  queue = [];
  emit();
}

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ');
}

function widthStyle(width: number | string | undefined): CSSProperties {
  const value = width ?? 416;
  return { width: typeof value === 'number' ? `${value}px` : value };
}

function modalStyle(width: number | string | undefined, style: CSSProperties | undefined) {
  return { ...style, ...widthStyle(width) };
}

function zIndexStyle(zIndex: number | undefined): CSSProperties {
  return zIndex === undefined ? {} : { zIndex };
}

function getPortalContainer(getContainer: LegacyConfirmContainer | undefined) {
  if (getContainer === false) return false;
  if (typeof document === 'undefined') return undefined;
  if (typeof getContainer === 'string') {
    return document.querySelector<HTMLElement>(getContainer) ?? document.body;
  }
  if (typeof getContainer === 'function') return getContainer();
  return getContainer ?? document.body;
}

function defaultIcon(type: LegacyConfirmType, iconType: LegacyConfirmIconType | undefined) {
  switch (iconType ?? type) {
    case 'success':
    case 'check-circle':
      return <LegacyCheckCircleIcon />;
    case 'error':
    case 'close-circle':
      return <LegacyCloseCircleIcon />;
    case 'info':
    case 'info-circle':
      return <LegacyInfoCircleIcon />;
    case 'warning':
    case 'exclamation-circle':
      return <LegacyExclamationCircleIcon />;
    case 'confirm':
    case 'question-circle':
    default:
      return <LegacyQuestionCircleIcon />;
  }
}

interface ConfirmActionButtonProps {
  actionFn: LegacyConfirmAction | undefined;
  autoFocus: boolean;
  buttonProps: LegacyConfirmButtonProps | undefined;
  children: ReactNode;
  closeModal: (...args: unknown[]) => void;
  type?: LegacyButtonProps['type'];
}

function ConfirmActionButton({
  actionFn,
  autoFocus,
  buttonProps,
  children,
  closeModal,
  type,
}: ConfirmActionButtonProps) {
  const [loading, setLoading] = useState(false);
  const buttonRef = useRef<HTMLButtonElement | HTMLAnchorElement | null>(null);

  useEffect(() => {
    if (!autoFocus) return;
    const timer = window.setTimeout(() => buttonRef.current?.focus());
    return () => window.clearTimeout(timer);
  }, [autoFocus]);

  const handleClick = () => {
    if (!actionFn) {
      closeModal();
      return;
    }

    const result = actionFn.length ? actionFn(closeModal) : actionFn();
    if (!actionFn.length && !result) {
      closeModal();
      return;
    }

    if (!isThenable(result)) return;
    setLoading(true);
    result.then(
      (...args) => closeModal(...args),
      (error) => {
        console.error(error);
        setLoading(false);
      },
    );
  };

  const mergedLoading =
    buttonProps && 'loading' in buttonProps ? buttonProps.loading : loading;

  return (
    <LegacyButton
      {...buttonProps}
      ref={buttonRef}
      loading={mergedLoading}
      type={type}
      onClick={handleClick}
    >
      {children}
    </LegacyButton>
  );
}

export function LegacyConfirmProvider() {
  const [requests, setRequests] = useState<LegacyConfirmRequest[]>(() => [...queue]);

  const open = requests.length > 0;

  useEffect(() => {
    const listener = () => {
      setRequests([...queue]);
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  useEffect(() => {
    if (!open || typeof document === 'undefined') return;

    const hadOpenClass = document.body.classList.contains('ant-modal-open');

    document.body.classList.add('ant-modal-open');

    return () => {
      if (!hadOpenClass) document.body.classList.remove('ant-modal-open');
    };
  }, [open]);

  if (!open || typeof document === 'undefined') return null;

  return (
    <>
      {requests.map((request) => (
        <LegacyConfirmDialog key={request.id} request={request} />
      ))}
    </>
  );
}

interface LegacyConfirmDialogProps {
  request: LegacyConfirmRequest;
}

function LegacyConfirmDialog({ request }: LegacyConfirmDialogProps) {
  const sentinelStartRef = useRef<HTMLDivElement>(null);
  const sentinelEndRef = useRef<HTMLDivElement>(null);
  const openedAtRef = useRef(0);
  const dialogMouseDownRef = useRef(false);

  const close = useCallback(
    (value: boolean, triggerCancel = false, ...args: unknown[]) => {
      if (!request) return;
      resolveRequest(request, value);
      queue = queue.filter((item) => item.id !== request.id);
      emit();
      if (triggerCancel) request.options.onCancel?.(...args);
    },
    [request],
  );

  useEffect(() => {
    openedAtRef.current = Date.now();
  }, [request.id]);

  const { options } = request;
  const prefixCls = options.prefixCls ?? 'ant-modal';
  const confirmPrefixCls = `${prefixCls}-confirm`;
  const modalType = options.type ?? 'confirm';
  const okCancel = options.okCancel ?? options.showCancel ?? true;
  const okText = options.okText ?? (okCancel ? '确定' : '知道了');
  const cancelText = options.cancelText ?? '取消';
  const okType = options.okType ?? 'primary';
  const mask = options.mask ?? true;
  const maskClosable = options.maskClosable ?? false;
  const keyboard = options.keyboard ?? true;
  const autoFocusButton =
    options.autoFocusButton === null ? false : options.autoFocusButton ?? 'ok';
  const wrapClassName = classNames(
    `${prefixCls}-wrap`,
    options.centered && `${prefixCls}-centered`,
    options.centered && `${confirmPrefixCls}-centered`,
  );
  const modalClassName = classNames(
    prefixCls,
    confirmPrefixCls,
    `${confirmPrefixCls}-${modalType}`,
    options.className,
  );
  const modalIcon =
    options.icon === undefined ? defaultIcon(modalType, options.iconType) : options.icon;

  const handleMaskClick = (event: MouseEvent<HTMLDivElement>) => {
    if (
      event.target === event.currentTarget &&
      mask &&
      maskClosable &&
      Date.now() - openedAtRef.current >= 300 &&
      !dialogMouseDownRef.current
    ) {
      close(false, true, { triggerCancel: true });
    }
  };

  const handleMaskMouseUp = () => {
    if (!dialogMouseDownRef.current) return;
    window.setTimeout(() => {
      dialogMouseDownRef.current = false;
    }, 0);
  };

  const handleDialogKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (keyboard && (event.key === 'Escape' || event.keyCode === 27)) {
      event.stopPropagation();
      close(false, true, { triggerCancel: true });
      return;
    }

    if (event.key !== 'Tab' && event.keyCode !== 9) return;
    const activeElement = document.activeElement;
    if (event.shiftKey && activeElement === sentinelStartRef.current) {
      event.preventDefault();
      sentinelEndRef.current?.focus();
      return;
    }
    if (!event.shiftKey && activeElement === sentinelEndRef.current) {
      event.preventDefault();
      sentinelStartRef.current?.focus();
    }
  };

  const sentinelStyle = {
    width: 0,
    height: 0,
    overflow: 'hidden',
    outline: 'none',
  };

  const root = (
    <div className={`${prefixCls}-root`}>
      {mask ? (
        <div
          className={`${prefixCls}-mask`}
          style={{ ...zIndexStyle(options.zIndex), ...options.maskStyle }}
        />
      ) : null}
      <div
        tabIndex={-1}
        className={wrapClassName}
        role="dialog"
        style={{ zIndex: options.zIndex }}
        onClick={handleMaskClick}
        onMouseUp={mask && maskClosable ? handleMaskMouseUp : undefined}
        onKeyDown={handleDialogKeyDown}
      >
        <div
          className={modalClassName}
          role="document"
          style={modalStyle(options.width, options.style)}
          onMouseDown={() => {
            dialogMouseDownRef.current = true;
          }}
        >
          <div
            tabIndex={0}
            ref={sentinelStartRef}
            style={sentinelStyle}
            aria-hidden="true"
          />
          <div className={`${prefixCls}-content`}>
            <div className={`${prefixCls}-body`}>
              <div className={`${confirmPrefixCls}-body-wrapper`}>
                <div className={`${confirmPrefixCls}-body`}>
                  {modalIcon}
                  {options.title === undefined ? null : (
                    <span className={`${confirmPrefixCls}-title`}>{options.title}</span>
                  )}
                  <div className={`${confirmPrefixCls}-content`}>{options.content}</div>
                </div>
                <div className={`${confirmPrefixCls}-btns`}>
                  {okCancel && (
                    <ConfirmActionButton
                      actionFn={options.onCancel}
                      autoFocus={autoFocusButton === 'cancel'}
                      buttonProps={options.cancelButtonProps}
                      closeModal={() => close(false)}
                    >
                      {cancelText}
                    </ConfirmActionButton>
                  )}
                  <ConfirmActionButton
                    actionFn={options.onOk}
                    autoFocus={autoFocusButton === 'ok'}
                    buttonProps={options.okButtonProps}
                    closeModal={() => close(true)}
                    type={okType}
                  >
                    {okText}
                  </ConfirmActionButton>
                </div>
              </div>
            </div>
          </div>
          <div tabIndex={0} ref={sentinelEndRef} style={sentinelStyle} aria-hidden="true" />
        </div>
      </div>
    </div>
  );

  const portalContainer = getPortalContainer(options.getContainer);
  return portalContainer === false ? root : createPortal(root, portalContainer ?? document.body);
}

function isThenable(value: unknown): value is PromiseLike<unknown> {
  return Boolean(value && typeof (value as PromiseLike<unknown>).then === 'function');
}
