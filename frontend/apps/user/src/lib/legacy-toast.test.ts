import { readFileSync } from 'node:fs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from './legacy-toast';

describe('legacy toast behavior', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = '';
    toast.dismiss();
  });

  afterEach(() => {
    toast.dismiss();
    vi.useRealTimers();
  });

  it('keeps only one legacy message notice', () => {
    toast.success('first');
    toast.error('second');

    const notices = document.querySelectorAll('.ant-message-notice');
    expect(notices).toHaveLength(1);
    expect(notices[0]?.textContent).toContain('second');
  });

  it('destroys message notices without closing notifications', () => {
    toast.loading('loading');
    toast.error('error', { description: 'details' });

    toast.destroy();

    expect(document.querySelectorAll('.ant-message-notice')).toHaveLength(0);
    expect(document.querySelectorAll('.ant-notification-notice')).toHaveLength(1);
  });

  it('allows desktop notifications to stack', () => {
    toast.error('error', { description: 'first' });
    toast.info('info', { description: 'second' });

    expect(document.querySelectorAll('.ant-notification-notice')).toHaveLength(2);
  });

  it('keeps legacy notification message and description text adjacent', () => {
    toast.error('Request failed', { description: 'Server Error' });

    expect(document.querySelector('.ant-notification-notice')?.textContent).toContain(
      'Request failedServer Error',
    );
  });

  // Guard: every antd Icon word must come from the shared locale registry (the single
  // source of truth), never a re-inlined per-locale `=== 'zh-CN' ? '图标' : 'icon'` copy.
  // antd v3's zh-CN-only Icon word was historically scattered across the icon surfaces;
  // both the imperative toast builder and the React loading icon now derive it from the
  // registry, exactly as components/ant-icon.tsx does. This locks that in.
  it('derives the antd Icon word from the shared registry, never an inline per-locale copy', () => {
    const sources = {
      'lib/legacy-toast.ts': readFileSync(`${process.cwd()}/src/lib/legacy-toast.ts`, 'utf8'),
      'components/legacy-loading-icon.tsx': readFileSync(
        `${process.cwd()}/src/components/legacy-loading-icon.tsx`,
        'utf8',
      ),
    };
    for (const [, source] of Object.entries(sources)) {
      expect(source).toContain('getLocaleAntdMessages');
      expect(source).toContain('.iconWord');
      expect(source).not.toContain("? '图标'");
      expect(source).not.toContain("'图标' : 'icon'");
    }
  });
});
