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

  it('allows desktop notifications to stack', () => {
    toast.error('error', { description: 'first' });
    toast.info('info', { description: 'second' });

    expect(document.querySelectorAll('.ant-notification-notice')).toHaveLength(2);
  });
});
