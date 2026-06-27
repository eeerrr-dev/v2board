import { createElement, type ComponentType } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import * as ToastPrimitive from '@radix-ui/react-toast';
import { CheckCircle, Info, LoaderCircle, X, XCircle } from 'lucide-react';
import { cn } from '@/lib/cn';

type ToastType = 'success' | 'error' | 'info' | 'loading';
type ToastKind = 'message' | 'notification';

interface ToastOptions {
  description?: string;
  duration?: number;
}

interface ToastEntry {
  id: number;
  type: ToastType;
  kind: ToastKind;
  message: string;
  description?: string;
  duration: number;
}

let root: Root | null = null;
let container: HTMLDivElement | null = null;
let nextToastId = 1;
let entries: ToastEntry[] = [];

export const toast = {
  success: (message: string, options?: ToastOptions) => openToast('success', message, options),
  error: (message: string, options?: ToastOptions) => openToast('error', message, options),
  info: (message: string, options?: ToastOptions) => openToast('info', message, options),
  loading: (message: string, options?: ToastOptions) => openToast('loading', message, options),
  destroy: () => destroyMessageToasts(),
  dismiss: (id?: number | string) => dismissToast(id),
};

function openToast(type: ToastType, message: string, options: ToastOptions = {}): number {
  ensureToastRoot();
  const id = nextToastId++;
  const kind: ToastKind = options.description && !isLegacyMobile() ? 'notification' : 'message';
  const duration = options.duration ?? (kind === 'notification' ? 1500 : 3000);
  const entry: ToastEntry = {
    id,
    type,
    kind,
    message,
    description: kind === 'notification' ? options.description : undefined,
    duration,
  };

  entries = kind === 'message'
    ? [...entries.filter((item) => item.kind !== 'message'), entry]
    : [...entries, entry];
  renderToasts();
  return id;
}

function dismissToast(id?: number | string): void {
  entries = id === undefined
    ? []
    : entries.filter((entry) => entry.id !== Number(id));
  renderToasts();
}

function destroyMessageToasts(): void {
  entries = entries.filter((entry) => entry.kind !== 'message');
  renderToasts();
}

function ensureToastRoot() {
  if (root && container?.isConnected) return;
  if (root && !container?.isConnected) {
    root.unmount();
    root = null;
    container = null;
  }
  container = document.createElement('div');
  container.className = 'v2board-toast-host';
  document.body.appendChild(container);
  root = createRoot(container);
}

function renderToasts() {
  root?.render(createElement(ToastHost, { entries, onDismiss: dismissToast }));
}

function ToastHost({
  entries: toastEntries,
  onDismiss,
}: {
  entries: ToastEntry[];
  onDismiss: (id: number) => void;
}) {
  return createElement(
    ToastPrimitive.Provider,
    { swipeDirection: 'right' },
    toastEntries.map((entry) =>
      createElement(ToastNotice, { key: entry.id, entry, onDismiss }),
    ),
    createElement(ToastPrimitive.Viewport, {
      className:
        'v2board-toast-viewport fixed top-4 right-4 z-[1200] flex w-[min(24rem,calc(100vw-2rem))] flex-col gap-3 outline-none',
    }),
  );
}

function ToastNotice({
  entry,
  onDismiss,
}: {
  entry: ToastEntry;
  onDismiss: (id: number) => void;
}) {
  const Icon = TOAST_ICON[entry.type];
  const toneClass = TOAST_TONE[entry.type];

  return createElement(
    ToastPrimitive.Root,
    {
      defaultOpen: true,
      duration: entry.duration,
      onOpenChange: (open: boolean) => {
        if (!open) onDismiss(entry.id);
      },
      className: cn(
        'v2board-toast-root grid grid-cols-[auto_1fr_auto] items-start gap-3 rounded-xl border border-border bg-card p-4 text-card-foreground shadow-lg data-[swipe=cancel]:translate-x-0 data-[swipe=end]:translate-x-[var(--radix-toast-swipe-end-x)] data-[swipe=move]:translate-x-[var(--radix-toast-swipe-move-x)]',
        entry.kind === 'message' && 'v2board-toast-message',
        entry.kind === 'notification' && 'v2board-toast-notification',
      ),
    },
    createElement(
      'span',
      {
        'aria-hidden': 'true',
        className: cn('flex size-9 items-center justify-center rounded-md border', toneClass),
      },
      createElement(Icon, {
        className: cn('size-5', entry.type === 'loading' && 'animate-spin'),
      }),
    ),
    createElement(
      'div',
      { className: 'min-w-0' },
      createElement(
        ToastPrimitive.Title,
        { className: 'text-sm leading-5 font-semibold' },
        entry.message,
      ),
      entry.description
        ? createElement(
            ToastPrimitive.Description,
            { className: 'mt-1 text-sm leading-5 text-muted-foreground' },
            entry.description,
          )
        : null,
    ),
    createElement(
      ToastPrimitive.Close,
      {
        'aria-label': 'Close',
        className:
          '-mt-1 -mr-1 flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
      },
      createElement(X, { 'aria-hidden': 'true', className: 'size-4' }),
    ),
  );
}

const TOAST_ICON: Record<ToastType, ComponentType<{ className?: string }>> = {
  success: CheckCircle,
  error: XCircle,
  info: Info,
  loading: LoaderCircle,
};

const TOAST_TONE: Record<ToastType, string> = {
  success: 'border-green-200 bg-green-50 text-green-700',
  error: 'border-destructive/30 bg-destructive/10 text-destructive',
  info: 'border-blue-200 bg-blue-50 text-blue-700',
  loading: 'border-blue-200 bg-blue-50 text-blue-700',
};

function isLegacyMobile(): boolean {
  return window.navigator.userAgent.toLowerCase().includes('mobile');
}
