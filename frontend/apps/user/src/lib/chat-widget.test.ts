import { afterEach, describe, expect, it } from 'vitest';
import { setRuntimeConfig } from '@/test/runtime-config';
import { installChatWidget } from './chat-widget';
import { reportUserInfoToChat } from './queries';
import type { UserInfo } from '@v2board/types';

function chatScripts(): HTMLScriptElement[] {
  return Array.from(document.querySelectorAll('script[data-v2board-chat-widget]'));
}

describe('chat-widget SDK loader (docs/api-dialect.md §10.6)', () => {
  afterEach(() => {
    for (const script of chatScripts()) script.remove();
    delete window.$crisp;
    delete window.CRISP_WEBSITE_ID;
    delete window.Tawk_API;
    delete window.Tawk_LoadStart;
    setRuntimeConfig();
  });

  it('stays inert when no chat_widget is injected (feature off default)', () => {
    setRuntimeConfig({});

    expect(installChatWidget()).toBeNull();
    expect(chatScripts()).toHaveLength(0);
    expect(window.$crisp).toBeUndefined();
    expect(window.CRISP_WEBSITE_ID).toBeUndefined();
    expect(window.Tawk_API).toBeUndefined();
  });

  it('stays inert for a partial or unknown provider shape', () => {
    setRuntimeConfig({
      chat_widget: { provider: 'tawk', property_id: 'abc123' } as never,
    });
    expect(installChatWidget()).toBeNull();

    setRuntimeConfig({
      chat_widget: { provider: 'zendesk', website_id: 'x' } as never,
    });
    expect(installChatWidget()).toBeNull();
    expect(chatScripts()).toHaveLength(0);
  });

  it('loads Crisp via the official queue globals plus the l.js dynamic script', () => {
    setRuntimeConfig({
      chat_widget: { provider: 'crisp', website_id: '01234567-89ab-cdef-0123-456789abcdef' },
    });

    expect(installChatWidget()).toBe('crisp');

    expect(window.CRISP_WEBSITE_ID).toBe('01234567-89ab-cdef-0123-456789abcdef');
    const scripts = chatScripts();
    expect(scripts).toHaveLength(1);
    expect(scripts[0]!.src).toBe('https://client.crisp.chat/l.js');
    expect(scripts[0]!.async).toBe(true);
    // Dynamic insertion only — never an inline snippet (no CSP inline allowance).
    expect(scripts[0]!.textContent).toBe('');

    // The frozen §2 session-data pushes in queries.ts reactivate against the
    // queue with their payloads byte-unchanged.
    reportUserInfoToChat({ email: 'user@example.com', balance: 1234 } as UserInfo);
    expect(window.$crisp).toEqual([
      ['set', 'user:email', 'user@example.com'],
      ['set', 'session:data', [[['Balance', 12.34]]]],
    ]);
  });

  it('loads Tawk via the official embed URL for the configured property/widget', () => {
    setRuntimeConfig({
      chat_widget: {
        provider: 'tawk',
        property_id: '0123456789abcdef01234567',
        widget_id: 'default',
      },
    });

    expect(installChatWidget()).toBe('tawk');

    expect(window.Tawk_API).toEqual({});
    expect(window.Tawk_LoadStart).toBeInstanceOf(Date);
    const scripts = chatScripts();
    expect(scripts).toHaveLength(1);
    expect(scripts[0]!.src).toBe('https://embed.tawk.to/0123456789abcdef01234567/default');
    expect(scripts[0]!.async).toBe(true);
    expect(scripts[0]!.textContent).toBe('');
  });

  it('installs at most one SDK script per document', () => {
    setRuntimeConfig({
      chat_widget: { provider: 'crisp', website_id: '01234567-89ab-cdef-0123-456789abcdef' },
    });

    expect(installChatWidget()).toBe('crisp');
    expect(installChatWidget()).toBe('crisp');

    expect(chatScripts()).toHaveLength(1);
  });
});
