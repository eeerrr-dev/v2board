import { getChatWidgetConfig, type ChatWidgetConfig } from './runtime-config';

// docs/api-dialect.md §10.6: the user SPA loads the configured provider's
// official SDK from the typed runtime `chat_widget` object as a
// 'self'-originated dynamic script insertion — never an inline snippet (no new
// CSP inline allowance). The frozen §2 session-data pushes in queries.ts
// reactivate against the SDK globals these loaders establish.

declare global {
  interface Window {
    /** Crisp official loader input: read by https://client.crisp.chat/l.js. */
    CRISP_WEBSITE_ID?: string;
    /** Tawk official loader input: the embed reads it to time widget startup. */
    Tawk_LoadStart?: Date;
  }
}

const SCRIPT_MARKER = 'data-v2board-chat-widget';

/**
 * Installs the configured chat widget once per document. Returns the provider
 * it activated, or null when no provider is completely configured (§10.6:
 * absent `chat_widget` = feature off, the default).
 */
export function installChatWidget(): ChatWidgetConfig['provider'] | null {
  const config = getChatWidgetConfig();
  if (!config) return null;
  if (document.querySelector(`script[${SCRIPT_MARKER}]`)) return config.provider;

  if (config.provider === 'crisp') {
    // Official Crisp embed contract: the queue array plus CRISP_WEBSITE_ID,
    // then the l.js loader. The array satisfies the frozen `$crisp.push`
    // session-data contract in queries.ts until the SDK replaces it.
    window.$crisp ??= [] as unknown[] as NonNullable<Window['$crisp']>;
    window.CRISP_WEBSITE_ID = config.website_id;
    appendSdkScript('https://client.crisp.chat/l.js');
    return 'crisp';
  }

  // Official Tawk embed contract: Tawk_API/Tawk_LoadStart globals, then the
  // per-property embed script.
  window.Tawk_API ??= {};
  window.Tawk_LoadStart = new Date();
  appendSdkScript(
    `https://embed.tawk.to/${encodeURIComponent(config.property_id)}/${encodeURIComponent(
      config.widget_id,
    )}`,
  );
  return 'tawk';
}

function appendSdkScript(src: string): void {
  const script = document.createElement('script');
  script.async = true;
  script.src = src;
  script.setAttribute(SCRIPT_MARKER, 'true');
  document.head.append(script);
}
