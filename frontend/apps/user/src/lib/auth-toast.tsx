import { toast, type ToastOptions } from './toast';

type AuthToastOptions = ToastOptions;

export const authToast = {
  success: (message: string, options?: AuthToastOptions) => toast.success(message, options),
  error: (message: string, options?: AuthToastOptions) => toast.error(message, options),
  dismiss: (id?: number | string) => toast.dismiss(id),
};
