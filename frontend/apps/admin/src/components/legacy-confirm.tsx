import { useCallback, useEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { LegacyQuestionCircleIcon, LegacyInfoCircleIcon, LegacyLoadingIcon } from './legacy-ant-icon';
import { LegacyButton } from './legacy-button';

interface LegacyConfirmOptions {
  title: ReactNode;
  content?: ReactNode;
  onOk?: LegacyConfirmAction;
  onCancel?: LegacyConfirmAction;
  okText?: ReactNode;
  cancelText?: ReactNode;
  maskClosable?: boolean;
  showCancel?: boolean;
  okButtonProps?: {
    disabled?: boolean;
    loading?: boolean;
  };
  centered?: boolean;
  type?: 'confirm' | 'info';
}

type LegacyConfirmAction = (...args: unknown[]) => unknown;

interface LegacyConfirmRequest {
  id: number;
  options: LegacyConfirmOptions;
  resolve: (value: boolean) => void;
}

let nextId = 1;
let queue: LegacyConfirmRequest[] = [];
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((listener) => listener());
}

function currentRequest() {
  return queue[0] ?? null;
}

export function legacyConfirm(options: LegacyConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    queue = [...queue, { id: nextId++, options, resolve }];
    emit();
  });
}

export function legacyInfo(options: Omit<LegacyConfirmOptions, 'showCancel' | 'type'>): Promise<boolean> {
  return legacyConfirm({ ...options, showCancel: false, type: 'info' });
}

export function LegacyConfirmProvider() {
  const [request, setRequest] = useState<LegacyConfirmRequest | null>(() => currentRequest());
  const [open, setOpen] = useState(() => Boolean(currentRequest()));
  const [actionLoading, setActionLoading] = useState(false);
  const bodyRef = useRef<HTMLDivElement>(null);

  const close = useCallback(
    (value: boolean) => {
      if (!request) return;
      request.resolve(value);
      queue = queue.filter((item) => item.id !== request.id);
      emit();
    },
    [request],
  );

  const runAction = useCallback(
    (action: LegacyConfirmAction | undefined, value: boolean) => {
      if (!action) {
        close(value);
        return;
      }
      const closeModal = () => close(value);
      const result = action.length ? action(closeModal) : action();
      if (!action.length && !result) {
        close(value);
        return;
      }
      if (isThenable(result)) {
        setActionLoading(true);
        result.then(
          () => close(value),
          (error) => {
            console.error(error);
            setActionLoading(false);
          },
        );
      }
    },
    [close],
  );

  useEffect(() => {
    const listener = () => {
      const nextRequest = currentRequest();
      if (nextRequest) {
        setRequest(nextRequest);
        setOpen(true);
      } else {
        setOpen(false);
      }
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  useEffect(() => {
    if (!open || typeof document === 'undefined') return;

    const hadOpenClass = document.body.classList.contains('ant-modal-open');
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') runAction(request?.options.onCancel, false);
    };

    document.body.classList.add('ant-modal-open');
    window.addEventListener('keydown', onKeyDown);
    bodyRef.current?.querySelector<HTMLButtonElement>('.ant-btn-primary')?.focus();

    return () => {
      if (!hadOpenClass) document.body.classList.remove('ant-modal-open');
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [open, request?.options.onCancel, runAction]);

  useEffect(() => {
    setActionLoading(false);
  }, [request?.id]);

  if (!open || !request || typeof document === 'undefined') return null;

  const { options } = request;
  const okButtonLoading = actionLoading || Boolean(options.okButtonProps?.loading);
  const okButtonClassName = `ant-btn ant-btn-primary${okButtonLoading ? ' ant-btn-loading' : ''}`;
  const modalType = options.type ?? 'confirm';
  const wrapClassName = `ant-modal-wrap${options.centered ? ' ant-modal-centered' : ''}`;
  const modalClassName = `ant-modal ant-modal-confirm ant-modal-confirm-${modalType}`;
  const modalIcon =
    modalType === 'info' ? <LegacyInfoCircleIcon /> : <LegacyQuestionCircleIcon />;

  const handleMaskClick = (event: MouseEvent<HTMLDivElement>) => {
    if (event.target === event.currentTarget && options.maskClosable) {
      runAction(options.onCancel, false);
    }
  };

  return createPortal(
    <div className="ant-modal-root">
      <div className="ant-modal-mask" />
      <div tabIndex={-1} className={wrapClassName} role="dialog" onClick={handleMaskClick}>
        <div
          className={modalClassName}
          role="document"
          style={{ width: '416px' }}
        >
          <div className="ant-modal-content">
            <div className="ant-modal-body">
              <div className="ant-modal-confirm-body-wrapper" ref={bodyRef}>
                <div className="ant-modal-confirm-body">
                  {modalIcon}
                  <span className="ant-modal-confirm-title">{options.title}</span>
                  <div className="ant-modal-confirm-content">{options.content}</div>
                </div>
                <div className="ant-modal-confirm-btns">
                  {options.showCancel !== false && (
                    <LegacyButton
                      type="button"
                      className="ant-btn"
                      onClick={() => runAction(options.onCancel, false)}
                    >
                      {options.cancelText ?? '取消'}
                    </LegacyButton>
                  )}
                  <LegacyButton
                    type="button"
                    className={okButtonClassName}
                    disabled={options.okButtonProps?.disabled}
                    onClick={() => runAction(options.onOk, true)}
                  >
                    {okButtonLoading ? <LegacyLoadingIcon /> : null}
                    {options.okText ?? '确定'}
                  </LegacyButton>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function isThenable(value: unknown): value is PromiseLike<unknown> {
  return Boolean(value && typeof (value as PromiseLike<unknown>).then === 'function');
}
