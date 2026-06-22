import { getCurrentLocale } from './errors';
import { ANT_ICONS, type AntIconName } from './ant-icons';

type ToastType = 'success' | 'error' | 'info' | 'loading';

interface ToastOptions {
  description?: string;
  duration?: number;
}

interface ToastEntry {
  node: HTMLElement;
  timer?: number;
  duration: number;
  // rc-notification transition prefix: "move-up" (ant-message) /
  // "ant-notification-fade" (ant-notification), plus the leave duration.
  transition: string;
  leaveMs: number;
}

let nextToastId = 1;
const entries = new Map<number, ToastEntry>();

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
  const isNotification = Boolean(options.description) && !isLegacyMobile();
  // antd v3 defaults: ant-message uses the message module default h=3 (3000ms);
  // ant-notification is opened by the original toast helper with an explicit
  // duration:1.5 (1500ms). A loading toast carries no explicit duration in the
  // original, so it falls back to ant-message's 3s as well.
  const duration = options.duration ?? (isNotification ? 1500 : 3000);
  const node = isNotification
    ? createNotification(type, message, options.description ?? '', () => dismissToast(id))
    : createMessage(type, options.description ?? message);
  // ant-message enters/leaves via the "move-up" transition (0.3s); ant-notification
  // via "ant-notification-fade" (0.2s). Reproduce rc-notification's class lifecycle.
  const transition = isNotification ? 'ant-notification-fade' : 'move-up';
  const leaveMs = isNotification ? 200 : 300;
  playEnter(node, transition);
  entries.set(id, { node, duration, transition, leaveMs });
  if (duration > 0) {
    const timer = window.setTimeout(() => dismissToast(id), duration);
    entries.set(id, { node, timer, duration, transition, leaveMs });
  }
  // rc-notification clears the auto-dismiss timer on hover and restarts it on leave.
  node.addEventListener('mouseenter', () => clearToastTimer(id));
  node.addEventListener('mouseleave', () => startToastTimer(id));
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
  entries.delete(key);
  playLeave(entry.node, entry.transition, entry.leaveMs);
}

function clearToastTimer(id: number): void {
  const entry = entries.get(id);
  if (entry?.timer) {
    window.clearTimeout(entry.timer);
    entry.timer = undefined;
  }
}

function startToastTimer(id: number): void {
  const entry = entries.get(id);
  if (entry && entry.duration > 0 && entry.timer === undefined) {
    entry.timer = window.setTimeout(() => dismissToast(id), entry.duration);
  }
}

function createMessage(type: ToastType, content: string): HTMLElement {
  const root = ensureMessageRoot();
  // The bundled app configures antd message with maxCount: 1 during startup.
  // That cap applies to ant-message notices only; desktop notifications still stack.
  removeMessageToastsImmediately();
  const notice = document.createElement('div');
  notice.className = 'ant-message-notice';
  notice.innerHTML = `
    <div class="ant-message-notice-content">
      <div class="ant-message-custom-content ant-message-${type}">
        ${messageIconHtml(type)}<span>${escapeHtml(content)}</span>
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
  notice.className = 'ant-notification-notice ant-notification-notice-closable';
  notice.innerHTML = `
    <div class="ant-notification-notice-content">
      <div class="ant-notification-notice-with-icon">
        ${notificationIconHtml(type)}<div class="ant-notification-notice-message">${escapeHtml(message)}</div><div class="ant-notification-notice-description">${escapeHtml(description)}</div>
      </div>
    </div>
    <a tabindex="0" class="ant-notification-notice-close">${notificationCloseHtml()}</a>
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

function destroyMessageToasts(): void {
  removeMessageToastsImmediately();
}

function removeMessageToastsImmediately(): void {
  for (const [id, entry] of [...entries]) {
    if (entry.node.classList.contains('ant-message-notice')) {
      if (entry.timer) window.clearTimeout(entry.timer);
      entry.node.remove();
      entries.delete(id);
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

// rc-notification toggles "<transition>-enter" then "<transition>-enter-active"
// across two animation frames (the enter state must paint before -active is added
// so the keyframe actually runs); leave mirrors it and removes the node afterwards.
function playEnter(node: HTMLElement, transition: string): void {
  node.classList.add(`${transition}-enter`);
  requestAnimationFrame(() => {
    requestAnimationFrame(() => node.classList.add(`${transition}-enter-active`));
  });
}

function playLeave(node: HTMLElement, transition: string, leaveMs: number): void {
  node.classList.remove(`${transition}-enter`, `${transition}-enter-active`);
  node.classList.add(`${transition}-leave`);
  requestAnimationFrame(() => {
    requestAnimationFrame(() => node.classList.add(`${transition}-leave-active`));
  });
  window.setTimeout(() => node.remove(), leaveMs);
}

// antd FILLED status icons for ant-message (createElement(Icon,{type,theme:"filled"})).
const MESSAGE_ICONS: Record<ToastType, AntIconName> = {
  success: 'check-circle',
  error: 'close-circle',
  info: 'info-circle',
  loading: 'loading',
};

// antd renders NOTIFICATION status icons as the OUTLINED `-o` types (a hollow
// ring) with no theme — unlike the FILLED ant-message icons. loading has no
// notification status icon and falls back to the message-style spinner.
const NOTIFICATION_ICONS: Record<Exclude<ToastType, 'loading'>, AntIconName> = {
  success: 'check-circle-o',
  error: 'close-circle-o',
  info: 'info-circle-o',
};

// antd v3 Icon DOM as an HTML string (the toast layer builds markup, not React
// nodes): <i aria-label="<word>: <name>" class="anticon anticon-<name> [extra]">
// <svg ...><path/>…</svg></i>. Mirrors components/ant-icon.tsx.
function antIconHtml(name: AntIconName, extraClass = ''): string {
  const { viewBox, paths } = ANT_ICONS[name];
  const word = getCurrentLocale() === 'zh-CN' ? '图标' : 'icon';
  const className = `anticon anticon-${name}${extraClass ? ` ${extraClass}` : ''}`;
  const svgClass = name === 'loading' ? ' class="anticon-spin"' : '';
  const svgPaths = paths.map((d) => `<path d="${d}" />`).join('');
  return `<i aria-label="${word}: ${name}" class="${className}"><svg${svgClass} viewBox="${viewBox}" focusable="false" data-icon="${name}" width="1em" height="1em" fill="currentColor" aria-hidden="true">${svgPaths}</svg></i>`;
}

function messageIconHtml(type: ToastType): string {
  return antIconHtml(MESSAGE_ICONS[type]);
}

function notificationIconHtml(type: ToastType): string {
  if (type === 'loading') return antIconHtml('loading', 'ant-notification-notice-icon');
  return antIconHtml(
    NOTIFICATION_ICONS[type],
    `ant-notification-notice-icon ant-notification-notice-icon-${type}`,
  );
}

// antd wraps the notice close icon in a `-close-x` span; the icon carries the
// `ant-notification-close-icon` class.
function notificationCloseHtml(): string {
  return `<span class="ant-notification-close-x">${antIconHtml('close', 'ant-notification-close-icon')}</span>`;
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
