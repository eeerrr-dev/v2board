import { Toaster as SonnerToaster, toast as sonnerToast } from 'sonner';
import { useDarkMode } from '@/lib/dark-mode';

type ToastType = 'success' | 'error' | 'info' | 'loading';
type ToastId = string | number;

interface ToastOptions {
  description?: string;
  duration?: number;
}

const MESSAGE_TOAST_ID = 'v2board-message-toast';
let activeMessageId: ToastId | undefined;

export const toast = {
  success: (message: string, options?: ToastOptions) => openToast('success', message, options),
  error: (message: string, options?: ToastOptions) => openToast('error', message, options),
  info: (message: string, options?: ToastOptions) => openToast('info', message, options),
  loading: (message: string, options?: ToastOptions) => openToast('loading', message, options),
  destroy: () => destroyMessageToast(),
  dismiss: (id?: ToastId) => dismissToast(id),
};

function openToast(type: ToastType, message: string, options: ToastOptions = {}): ToastId {
  const notification = Boolean(options.description && !isMobileUserAgent());

  const id = sonnerToast[type](message, {
    className: notification ? 'v2board-toast-notification' : 'v2board-toast-message',
    description: notification ? options.description : undefined,
    duration: options.duration ?? (notification ? 1500 : 3000),
    id: notification ? undefined : MESSAGE_TOAST_ID,
  });

  if (!notification) activeMessageId = MESSAGE_TOAST_ID;
  return id;
}

function dismissToast(id?: ToastId): void {
  sonnerToast.dismiss(id);
  if (id === undefined || id === activeMessageId) activeMessageId = undefined;
}

function destroyMessageToast(): void {
  if (activeMessageId === undefined) return;
  sonnerToast.dismiss(activeMessageId);
  activeMessageId = undefined;
}

export function Toaster() {
  const theme = useDarkMode() ? 'dark' : 'light';

  return (
    <SonnerToaster
      closeButton
      richColors
      theme={theme}
      position="top-right"
      toastOptions={{
        classNames: {
          toast: 'v2board-toast-root',
          title: 'text-sm leading-5 font-semibold',
          description: 'text-sm leading-5 text-muted-foreground',
          closeButton: 'v2board-toast-close',
        },
      }}
    />
  );
}

function isMobileUserAgent(): boolean {
  return window.navigator.userAgent.toLowerCase().includes('mobile');
}

export type { ToastOptions };
