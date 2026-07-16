import { useRef, useState, useSyncExternalStore, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { SelectorParam } from 'i18next';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
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

function notify() {
  listeners.forEach((listener) => listener());
}

// Exposes the queue as an external store the same way lib/auth.ts and
// lib/dark-mode.ts do, so the provider can read the head request through
// useSyncExternalStore instead of mirroring it into local state.
function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): ConfirmDialogRequest | null {
  // Stable reference between renders: queue[0] only changes when queue is
  // reassigned (enqueue/close), so useSyncExternalStore never loops.
  return queue[0] ?? null;
}

export function confirmDialog(options: ConfirmDialogOptions): Promise<boolean> {
  return new Promise((resolve) => {
    queue = [...queue, { id: nextId++, options, resolve }];
    notify();
  });
}

export function ConfirmDialogProvider() {
  const { t } = useTranslation();
  const request = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const open = request !== null;
  const [actionLoading, setActionLoading] = useState(false);
  const closingRef = useRef(false);

  const close = (value: boolean) => {
    if (!request) return;
    closingRef.current = true;
    setActionLoading(false);
    request.resolve(value);
    queue = queue.filter((item) => item.id !== request.id);
    notify();
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
  const defaultText = getConfirmDialogDefaultText((selector) => t(selector));

  return (
    <AlertDialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !closingRef.current) cancel();
      }}
    >
      <AlertDialogContent
        className="sm:max-w-[26rem]"
        {...(options?.description ? {} : { 'aria-describedby': undefined })}
      >
        <AlertDialogHeader>
          <AlertDialogTitle>{options?.title}</AlertDialogTitle>
          {options?.description ? (
            <AlertDialogDescription>{options.description}</AlertDialogDescription>
          ) : null}
        </AlertDialogHeader>
        <AlertDialogFooter>
          {options?.showCancel !== false && (
            <AlertDialogCancel asChild>
              <Button
                type="button"
                variant="outline"
                disabled={actionLoading}
                onClick={(event) => {
                  event.preventDefault();
                  cancel();
                }}
              >
                {options?.cancelText ?? defaultText.cancelText}
              </Button>
            </AlertDialogCancel>
          )}
          <AlertDialogAction asChild>
            <Button
              type="button"
              disabled={options?.confirmButtonProps?.disabled}
              loading={buttonLoading}
              onClick={(event) => {
                event.preventDefault();
                void runAction(options?.onConfirm, true);
              }}
            >
              {options?.confirmText ?? defaultText.confirmText}
            </Button>
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export function getConfirmDialogDefaultText(translate: (selector: SelectorParam) => string) {
  return {
    confirmText: translate(($) => $.common.confirm),
    cancelText: translate(($) => $.common.cancel),
  };
}

export type { ConfirmDialogOptions };
