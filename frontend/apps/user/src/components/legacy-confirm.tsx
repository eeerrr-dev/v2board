import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';

interface LegacyConfirmOptions {
  title: ReactNode;
  content?: ReactNode;
  onOk?: LegacyConfirmAction;
  onCancel?: LegacyConfirmAction;
  okText?: ReactNode;
  cancelText?: ReactNode;
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
  const okButtonRef = useRef<HTMLButtonElement>(null);
  const triggerCancelRef = useRef<LegacyConfirmAction | null>(null);

  useEffect(() => {
    const listener = () => {
      const nextRequest = currentRequest();
      if (nextRequest) {
        setRequest(nextRequest);
        setOpen(true);
      } else {
        setOpen(false);
        setRequest(null);
      }
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  useEffect(() => {
    if (request && open) okButtonRef.current?.focus();
  }, [open, request]);

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

  const afterCancelClose = () => {
    const action = triggerCancelRef.current;
    triggerCancelRef.current = null;
    action?.({ triggerCancel: true });
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
  const defaultText = getLegacyConfirmDefaultText(i18n.language);

  return (
    <AlertDialog
      open={open}
      onOpenChange={(open) => {
        if (!open) {
          triggerCancel();
          afterCancelClose();
        }
      }}
    >
      <AlertDialogContent className="v2board-confirm-dialog sm:max-w-[26rem]">
        <AlertDialogHeader>
          <AlertDialogTitle className="v2board-confirm-title">
            {options?.title}
          </AlertDialogTitle>
          {options?.content ? (
            <AlertDialogDescription className="v2board-confirm-content">
              {options.content}
            </AlertDialogDescription>
          ) : null}
        </AlertDialogHeader>
        <AlertDialogFooter className="v2board-confirm-footer">
          {options?.showCancel !== false && (
            <Button
              type="button"
              variant="outline"
              onClick={() => runAction(options?.onCancel, false)}
            >
              {options?.cancelText ?? defaultText.cancelText}
            </Button>
          )}
          <Button
            ref={okButtonRef}
            type="button"
            className="v2board-confirm-primary"
            disabled={options?.okButtonProps?.disabled}
            loading={okButtonLoading}
            onClick={() => runAction(options?.onOk, true)}
          >
            {options?.okText ?? defaultText.okText}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
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
