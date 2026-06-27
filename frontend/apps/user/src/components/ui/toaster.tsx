import * as ToastPrimitive from '@radix-ui/react-toast';
import { CheckCircle, Info, LoaderCircle, X, XCircle } from 'lucide-react';
import { useSyncExternalStore, type ComponentType } from 'react';
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

let nextToastId = 1;
let entries: ToastEntry[] = [];
const listeners = new Set<() => void>();

export const toast = {
  success: (message: string, options?: ToastOptions) => openToast('success', message, options),
  error: (message: string, options?: ToastOptions) => openToast('error', message, options),
  info: (message: string, options?: ToastOptions) => openToast('info', message, options),
  loading: (message: string, options?: ToastOptions) => openToast('loading', message, options),
  destroy: () => destroyMessageToasts(),
  dismiss: (id?: number | string) => dismissToast(id),
};

function openToast(type: ToastType, message: string, options: ToastOptions = {}): number {
  const id = nextToastId++;
  const kind: ToastKind = options.description && !isMobileUserAgent() ? 'notification' : 'message';
  const entry: ToastEntry = {
    id,
    type,
    kind,
    message,
    description: kind === 'notification' ? options.description : undefined,
    duration: options.duration ?? (kind === 'notification' ? 1500 : 3000),
  };

  entries = kind === 'message'
    ? [...entries.filter((item) => item.kind !== 'message'), entry]
    : [...entries, entry];
  emit();
  return id;
}

function dismissToast(id?: number | string): void {
  entries = id === undefined
    ? []
    : entries.filter((entry) => entry.id !== Number(id));
  emit();
}

function destroyMessageToasts(): void {
  entries = entries.filter((entry) => entry.kind !== 'message');
  emit();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot() {
  return entries;
}

function emit() {
  listeners.forEach((listener) => listener());
}

export function Toaster() {
  const toastEntries = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return (
    <ToastPrimitive.Provider swipeDirection="right">
      {toastEntries.map((entry) => (
        <ToastNotice entry={entry} key={entry.id} onDismiss={dismissToast} />
      ))}
      <ToastPrimitive.Viewport className="v2board-toast-viewport fixed top-4 right-4 z-[1200] flex w-[min(24rem,calc(100vw-2rem))] flex-col gap-3 outline-none" />
    </ToastPrimitive.Provider>
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

  return (
    <ToastPrimitive.Root
      defaultOpen
      duration={entry.duration}
      onOpenChange={(open) => {
        if (!open) onDismiss(entry.id);
      }}
      className={cn(
        'v2board-toast-root grid grid-cols-[auto_1fr_auto] items-start gap-3 rounded-xl border border-border bg-card p-4 text-card-foreground shadow-lg data-[swipe=cancel]:translate-x-0 data-[swipe=end]:translate-x-[var(--radix-toast-swipe-end-x)] data-[swipe=move]:translate-x-[var(--radix-toast-swipe-move-x)]',
        entry.kind === 'message' && 'v2board-toast-message',
        entry.kind === 'notification' && 'v2board-toast-notification',
      )}
    >
      <span
        aria-hidden="true"
        className={cn('flex size-9 items-center justify-center rounded-md border', toneClass)}
      >
        <Icon className={cn('size-5', entry.type === 'loading' && 'animate-spin')} />
      </span>
      <div className="min-w-0">
        <ToastPrimitive.Title className="text-sm leading-5 font-semibold">
          {entry.message}
        </ToastPrimitive.Title>
        {entry.description ? (
          <ToastPrimitive.Description className="mt-1 text-sm leading-5 text-muted-foreground">
            {entry.description}
          </ToastPrimitive.Description>
        ) : null}
      </div>
      <ToastPrimitive.Close
        aria-label="Close"
        className="-mt-1 -mr-1 flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
      >
        <X aria-hidden="true" className="size-4" />
      </ToastPrimitive.Close>
    </ToastPrimitive.Root>
  );
}

const TOAST_ICON: Record<ToastType, ComponentType<{ className?: string }>> = {
  success: CheckCircle,
  error: XCircle,
  info: Info,
  loading: LoaderCircle,
};

const TOAST_TONE: Record<ToastType, string> = {
  success: 'border-emerald-200 bg-emerald-50 text-emerald-700',
  error: 'border-destructive/30 bg-destructive/10 text-destructive',
  info: 'border-sky-200 bg-sky-50 text-sky-700',
  loading: 'border-sky-200 bg-sky-50 text-sky-700',
};

function isMobileUserAgent(): boolean {
  return window.navigator.userAgent.toLowerCase().includes('mobile');
}

export type { ToastOptions };
