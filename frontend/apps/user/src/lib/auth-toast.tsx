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
      <Toast.Viewport className="v2board-auth-toast-viewport tw:fixed tw:right-4 tw:top-4 tw:z-[1200] tw:flex tw:w-[min(24rem,calc(100vw-2rem))] tw:flex-col tw:gap-3 tw:outline-none" />
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
      : 'tw:border-destructive/30 tw:bg-destructive-subtle tw:text-destructive';

  return (
    <Toast.Root
      defaultOpen
      duration={entry.duration}
      onOpenChange={(open) => {
        if (!open) onDismiss(entry.id);
      }}
      className="v2board-auth-toast-root tw:grid tw:grid-cols-[auto_1fr_auto] tw:items-start tw:gap-3 tw:rounded-card tw:border tw:border-border tw:bg-surface tw:p-4 tw:text-foreground tw:shadow-card tw:ring-1 tw:ring-border data-[swipe=move]:tw:translate-x-[var(--radix-toast-swipe-move-x)] data-[swipe=cancel]:tw:translate-x-0 data-[swipe=end]:tw:translate-x-[var(--radix-toast-swipe-end-x)]"
    >
      <span
        aria-hidden="true"
        className={`tw:flex tw:size-9 tw:items-center tw:justify-center tw:rounded-field tw:border ${toneClass}`}
      >
        <Icon className="tw:size-5" />
      </span>
      <div className="tw:min-w-0">
        <Toast.Title className="tw:text-sm tw:font-semibold tw:leading-5">
          {entry.message}
        </Toast.Title>
        {entry.description ? (
          <Toast.Description className="tw:mt-1 tw:text-sm tw:leading-5 tw:text-foreground-muted">
            {entry.description}
          </Toast.Description>
        ) : null}
      </div>
      <Toast.Close
        aria-label="Close"
        className="tw:-mr-1 tw:-mt-1 tw:flex tw:size-7 tw:items-center tw:justify-center tw:rounded-field tw:text-foreground-muted tw:transition tw:hover:bg-muted tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40"
      >
        <X aria-hidden="true" className="tw:size-4" />
      </Toast.Close>
    </Toast.Root>
  );
}
