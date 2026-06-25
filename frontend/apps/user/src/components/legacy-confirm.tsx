import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { AntBtn } from '@/components/ant-btn';
import { QuestionCircleIcon } from '@/components/ant-icon';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';

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

export function LegacyConfirmProvider() {
  const { i18n } = useTranslation();
  const [request, setRequest] = useState<LegacyConfirmRequest | null>(() => currentRequest());
  const [open, setOpen] = useState(() => Boolean(currentRequest()));
  const [actionLoading, setActionLoading] = useState(false);
  const bodyRef = useRef<HTMLDivElement>(null);
  const triggerCancelRef = useRef<LegacyConfirmAction | null>(null);

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

  // antd Modal.confirm defaults autoFocusButton:"ok" — focus the OK button each time a
  // confirm opens. This parent effect runs after DialogContent has focused the dialog wrap,
  // so the OK button keeps focus.
  useEffect(() => {
    if (request) bodyRef.current?.querySelector<HTMLButtonElement>('.ant-btn-primary')?.focus();
  }, [request]);

  useEffect(() => {
    setActionLoading(false);
  }, [request?.id]);

  const close = (value: boolean) => {
    if (!request) return;
    request.resolve(value);
    queue = queue.filter((item) => item.id !== request.id);
    emit();
  };

  const triggerCancel = () => {
    if (!request || !open) return;
    triggerCancelRef.current = request.options.onCancel ?? null;
    close(false);
  };

  const afterClose = () => {
    const action = triggerCancelRef.current;
    triggerCancelRef.current = null;
    action?.({ triggerCancel: true });
    if (!currentRequest()) setRequest(null);
  };

  const options = request?.options;
  // antd's ActionButton closes immediately for falsy action results, waits for thenables,
  // deliberately stays open for truthy non-thenables, and passes closeModal to arity > 0
  // callbacks without auto-closing them.
  const runAction = (action: LegacyConfirmAction | undefined, value: boolean) => {
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
  };

  const okButtonLoading = actionLoading || Boolean(options?.okButtonProps?.loading);
  const okButtonClassName = `ant-btn ant-btn-primary${okButtonLoading ? ' ant-btn-loading' : ''}`;
  const defaultText = getLegacyConfirmDefaultText(i18n.language);

  return (
    <Dialog
      open={open}
      onOpenChange={(open) => {
        if (!open) triggerCancel();
      }}
    >
      <DialogContent
        closable={false}
        footer={null}
        width={416}
        maskClosable={Boolean(options?.maskClosable)}
        afterClose={afterClose}
        className="ant-modal-confirm ant-modal-confirm-confirm"
      >
        <div className="ant-modal-confirm-body-wrapper" ref={bodyRef}>
          <div className="ant-modal-confirm-body">
            <QuestionCircleIcon />
            <span className="ant-modal-confirm-title">{options?.title}</span>
            <div className="ant-modal-confirm-content">{options?.content}</div>
          </div>
          <div className="ant-modal-confirm-btns">
            {options?.showCancel !== false && (
              <AntBtn
                type="button"
                className="ant-btn"
                onClick={() => runAction(options?.onCancel, false)}
              >
                {options?.cancelText ?? defaultText.cancelText}
              </AntBtn>
            )}
            <AntBtn
              type="button"
              className={okButtonClassName}
              disabled={options?.okButtonProps?.disabled}
              onClick={() => runAction(options?.onOk, true)}
            >
              {okButtonLoading ? <LegacyLoadingIcon /> : null}
              {options?.okText ?? defaultText.okText}
            </AntBtn>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function isThenable(value: unknown): value is PromiseLike<unknown> {
  return Boolean(value && typeof (value as PromiseLike<unknown>).then === 'function');
}

// antd Modal.confirm OK/Cancel text comes from the antd locale pack; sourced from
// the shared registry (en-US fallback for unknown locales, matching antd).
export function getLegacyConfirmDefaultText(locale: string | undefined) {
  const { okText, cancelText } = getLocaleAntdMessages(locale);
  return { okText, cancelText };
}
