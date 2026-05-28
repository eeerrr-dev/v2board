type ToastType = 'success' | 'error' | 'info' | 'loading';

interface ToastOptions {
  description?: string;
  duration?: number;
}

interface ToastEntry {
  node: HTMLElement;
  timer?: number;
}

let nextToastId = 1;
const entries = new Map<number, ToastEntry>();

export const toast = {
  success: (message: string, options?: ToastOptions) => openToast('success', message, options),
  error: (message: string, options?: ToastOptions) => openToast('error', message, options),
  info: (message: string, options?: ToastOptions) => openToast('info', message, options),
  loading: (message: string, options?: ToastOptions) => openToast('loading', message, options),
  dismiss: (id?: number | string) => dismissToast(id),
};

function openToast(type: ToastType, message: string, options: ToastOptions = {}): number {
  const id = nextToastId++;
  const duration = options.duration ?? (type === 'loading' ? 0 : 1500);
  const node =
    options.description && !isLegacyMobile()
      ? createNotification(type, message, options.description, () => dismissToast(id))
      : createMessage(type, options.description ?? message);
  entries.set(id, { node });
  if (duration > 0) {
    const timer = window.setTimeout(() => dismissToast(id), duration);
    entries.set(id, { node, timer });
  }
  return id;
}

function dismissToast(id?: number | string): void {
  if (id === undefined) {
    for (const key of entries.keys()) dismissToast(key);
    return;
  }
  const key = Number(id);
  const entry = entries.get(key);
  if (!entry) return;
  if (entry.timer) window.clearTimeout(entry.timer);
  entry.node.remove();
  entries.delete(key);
}

function createMessage(type: ToastType, content: string): HTMLElement {
  const root = ensureMessageRoot();
  dismissMessageToasts();
  const notice = document.createElement('div');
  notice.className = 'ant-message-notice';
  notice.innerHTML = `
    <div class="ant-message-notice-content">
      <div class="ant-message-custom-content ant-message-${type}">
        ${iconHtml(type)}<span>${escapeHtml(content)}</span>
      </div>
    </div>
  `;
  root.appendChild(notice);
  return notice;
}

function createNotification(
  type: ToastType,
  message: string,
  description: string,
  onClose: () => void,
): HTMLElement {
  const root = ensureNotificationRoot();
  const notice = document.createElement('div');
  notice.className = 'ant-notification-notice ant-notification-notice-closable ant-notification-notice-with-icon';
  notice.innerHTML = `
    ${iconHtml(type, 'ant-notification-notice-icon')}
    <div class="ant-notification-notice-message">${escapeHtml(message)}</div>
    <div class="ant-notification-notice-description">${escapeHtml(description)}</div>
    <a tabindex="0" class="ant-notification-notice-close">${iconHtml('close')}</a>
  `;
  notice.querySelector('.ant-notification-notice-close')?.addEventListener('click', onClose);
  root.appendChild(notice);
  return notice;
}

function ensureMessageRoot(): HTMLElement {
  let root = document.querySelector<HTMLElement>('.ant-message');
  if (!root) {
    root = document.createElement('div');
    root.className = 'ant-message';
    document.body.appendChild(root);
  }
  return root;
}

function dismissMessageToasts(): void {
  for (const [id, entry] of [...entries]) {
    if (entry.node.classList.contains('ant-message-notice')) {
      dismissToast(id);
    }
  }
}

function ensureNotificationRoot(): HTMLElement {
  let root = document.querySelector<HTMLElement>('.ant-notification.ant-notification-topRight');
  if (!root) {
    root = document.createElement('div');
    root.className = 'ant-notification ant-notification-topRight';
    root.style.top = '24px';
    root.style.right = '0px';
    document.body.appendChild(root);
  }
  return root;
}

function iconHtml(type: ToastType | 'close', extraClass = ''): string {
  if (type === 'close') return '<i class="anticon anticon-close ant-notification-close-icon"></i>';
  const icon = type === 'success'
    ? 'check-circle'
    : type === 'error'
      ? 'close-circle'
      : type === 'loading'
        ? 'loading'
        : 'info-circle';
  const typedClass = extraClass ? `${extraClass}-${type}` : '';
  return `<i class="anticon anticon-${icon} ${extraClass} ${typedClass}"></i>`;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function isLegacyMobile(): boolean {
  return window.navigator.userAgent.toLowerCase().includes('mobile');
}
