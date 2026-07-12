import { Toaster as SonnerToaster, toast as sonnerToast } from 'sonner';
import { useDarkMode } from '@/lib/dark-mode';

type ToastType = 'success' | 'error' | 'info' | 'loading';
type ToastId = string | number;

interface ToastOptions {
  description?: string;
  duration?: number;
}

export const toast = {
  success: (message: string, options?: ToastOptions) => openToast('success', message, options),
  error: (message: string, options?: ToastOptions) => openToast('error', message, options),
  info: (message: string, options?: ToastOptions) => openToast('info', message, options),
  loading: (message: string, options?: ToastOptions) => openToast('loading', message, options),
  dismiss: (id?: ToastId) => sonnerToast.dismiss(id),
};

function openToast(type: ToastType, message: string, options: ToastOptions = {}): ToastId {
  return sonnerToast[type](message, {
    description: options.description,
    ...(options.duration === undefined ? {} : { duration: options.duration }),
  });
}

export function Toaster() {
  const theme = useDarkMode() ? 'dark' : 'light';

  return (
    <SonnerToaster
      closeButton
      richColors
      theme={theme}
      position="bottom-right"
      toastOptions={{
        classNames: {
          title: 'text-sm leading-5 font-semibold',
          description: 'text-sm leading-5 text-muted-foreground',
        },
      }}
    />
  );
}

export type { ToastOptions };
