import { createRoot, type Root } from 'react-dom/client';
import * as Toast from '@radix-ui/react-toast';
import { CheckCircle, X, XCircle } from 'lucide-react';

type AuthToastType = 'success' | 'error';

interface AuthToastOptions {
  description?: string;
  duration?: number;
}

interface AuthToastEntry {
  id: number;
  type: AuthToastType;
  message: string;
  description?: string;
  duration: number;
}

let root: Root | null = null;
let container: HTMLDivElement | null = null;
let nextToastId = 1;
let entries: AuthToastEntry[] = [];

export const authToast = {
  success: (message: string, options?: AuthToastOptions) => openToast('success', message, options),
  error: (message: string, options?: AuthToastOptions) => openToast('error', message, options),
  dismiss: (id?: number) => dismissToast(id),
};

function openToast(type: AuthToastType, message: string, options: AuthToastOptions = {}) {
  ensureToastRoot();
  const id = nextToastId++;
  entries = [
    ...entries,
    {
      id,
      type,
      message,
      description: options.description,
      duration: options.duration ?? 3200,
    },
  ];
  renderToasts();
  return id;
}

function dismissToast(id?: number) {
  entries = id === undefined ? [] : entries.filter((entry) => entry.id !== id);
  renderToasts();
}

function ensureToastRoot() {
  if (root && container?.isConnected) return;
  if (root && !container?.isConnected) {
    // The previous host was detached (e.g. teardown / HMR). Unmount its React root before discarding
    // it — otherwise the stale tree leaks and React 19 warns when a new root mounts. Safe to call
    // synchronously here: ensureToastRoot only runs from imperative authToast.* calls, never inside a
    // React render.
    root.unmount();
    root = null;
    container = null;
  }
  container = document.createElement('div');
  container.className = 'v2board-auth-toast-host';
  document.body.appendChild(container);
  root = createRoot(container);
}

function renderToasts() {
  root?.render(<AuthToastHost entries={entries} onDismiss={dismissToast} />);
}

function AuthToastHost({
  entries: toastEntries,
  onDismiss,
}: {
  entries: AuthToastEntry[];
  onDismiss: (id: number) => void;
}) {
  return (
    <Toast.Provider swipeDirection="right">
      {toastEntries.map((entry) => (
        <AuthToast key={entry.id} entry={entry} onDismiss={onDismiss} />
      ))}
      <Toast.Viewport className="v2board-auth-toast-viewport fixed top-4 right-4 z-[1200] flex w-[min(24rem,calc(100vw-2rem))] flex-col gap-3 outline-none" />
    </Toast.Provider>
  );
}

function AuthToast({
  entry,
  onDismiss,
}: {
  entry: AuthToastEntry;
  onDismiss: (id: number) => void;
}) {
  const Icon = entry.type === 'success' ? CheckCircle : XCircle;
  const toneClass =
    entry.type === 'success'
      ? 'v2board-auth-toast-icon-success'
      : 'border-destructive/30 bg-destructive/10 text-destructive';

  return (
    <Toast.Root
      defaultOpen
      duration={entry.duration}
      onOpenChange={(open) => {
        if (!open) onDismiss(entry.id);
      }}
        className="v2board-auth-toast-root grid grid-cols-[auto_1fr_auto] items-start gap-3 rounded-xl border border-border bg-card p-4 text-card-foreground shadow-lg data-[swipe=cancel]:translate-x-0 data-[swipe=end]:translate-x-[var(--radix-toast-swipe-end-x)] data-[swipe=move]:translate-x-[var(--radix-toast-swipe-move-x)]"
    >
      <span
        aria-hidden="true"
        className={`flex size-9 items-center justify-center rounded-md border ${toneClass}`}
      >
        <Icon className="size-5" />
      </span>
      <div className="min-w-0">
        <Toast.Title className="text-sm leading-5 font-semibold">
          {entry.message}
        </Toast.Title>
        {entry.description ? (
          <Toast.Description className="mt-1 text-sm leading-5 text-muted-foreground">
            {entry.description}
          </Toast.Description>
        ) : null}
      </div>
      <Toast.Close
        aria-label="Close"
        className="-mt-1 -mr-1 flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
      >
        <X aria-hidden="true" className="size-4" />
      </Toast.Close>
    </Toast.Root>
  );
}
