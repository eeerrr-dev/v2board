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

interface ConfirmDialogOptions {
  title: ReactNode;
  description?: ReactNode;
  onConfirm?: ConfirmDialogAction;
  onCancel?: ConfirmDialogAction;
  confirmText?: ReactNode;
  cancelText?: ReactNode;
  showCancel?: boolean;
  confirmButtonProps?: {
    disabled?: boolean;
    loading?: boolean;
  };
}

type ConfirmDialogAction = () => unknown;

interface ConfirmDialogRequest {
  id: number;
  options: ConfirmDialogOptions;
  resolve: (value: boolean) => void;
}

let nextId = 1;
let queue: ConfirmDialogRequest[] = [];
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((listener) => listener());
}

function currentRequest() {
  return queue[0] ?? null;
}

export function confirmDialog(options: ConfirmDialogOptions): Promise<boolean> {
  return new Promise((resolve) => {
    queue = [...queue, { id: nextId++, options, resolve }];
    emit();
  });
}

export function ConfirmDialogProvider() {
  const { i18n } = useTranslation();
  const [request, setRequest] = useState<ConfirmDialogRequest | null>(() => currentRequest());
  const [open, setOpen] = useState(() => Boolean(currentRequest()));
  const [actionLoading, setActionLoading] = useState(false);
  const confirmButtonRef = useRef<HTMLButtonElement>(null);
  const closingRef = useRef(false);

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
    if (request && open) confirmButtonRef.current?.focus();
  }, [open, request]);

  useEffect(() => {
    setActionLoading(false);
  }, [request?.id]);

  const close = (value: boolean) => {
    if (!request) return;
    closingRef.current = true;
    request.resolve(value);
    queue = queue.filter((item) => item.id !== request.id);
    emit();
    queueMicrotask(() => {
      closingRef.current = false;
    });
  };

  const runAction = async (action: ConfirmDialogAction | undefined, value: boolean) => {
    if (!request || actionLoading) return;
    setActionLoading(true);
    try {
      await action?.();
      close(value);
    } catch (error) {
      console.error(error);
      setActionLoading(false);
    }
  };

  const cancel = () => void runAction(request?.options.onCancel, false);
  const options = request?.options;
  const buttonLoading = actionLoading || Boolean(options?.confirmButtonProps?.loading);
  const defaultText = getConfirmDialogDefaultText(i18n.language);

  return (
    <AlertDialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !closingRef.current) cancel();
      }}
    >
      <AlertDialogContent className="v2board-confirm-dialog sm:max-w-[26rem]">
        <AlertDialogHeader>
          <AlertDialogTitle className="v2board-confirm-title">
            {options?.title}
          </AlertDialogTitle>
          {options?.description ? (
            <AlertDialogDescription className="v2board-confirm-content">
              {options.description}
            </AlertDialogDescription>
          ) : null}
        </AlertDialogHeader>
        <AlertDialogFooter className="v2board-confirm-footer">
          {options?.showCancel !== false && (
            <Button type="button" variant="outline" onClick={cancel} disabled={actionLoading}>
              {options?.cancelText ?? defaultText.cancelText}
            </Button>
          )}
          <Button
            ref={confirmButtonRef}
            type="button"
            className="v2board-confirm-primary"
            disabled={options?.confirmButtonProps?.disabled}
            loading={buttonLoading}
            onClick={() => void runAction(options?.onConfirm, true)}
          >
            {options?.confirmText ?? defaultText.confirmText}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export function getConfirmDialogDefaultText(locale: string | undefined) {
  const { okText, cancelText } = getLocaleAntdMessages(locale);
  return { confirmText: okText, cancelText };
}

export type { ConfirmDialogOptions };
