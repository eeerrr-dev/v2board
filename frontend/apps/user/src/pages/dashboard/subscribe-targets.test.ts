import { afterEach, describe, expect, it, vi } from 'vitest';
import { getSubscribeTargets } from './subscribe-menu';

const IPAD_OS13_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15';
const MAC_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';

function setNavigator(userAgent: string, maxTouchPoints: number) {
  vi.spyOn(window.navigator, 'userAgent', 'get').mockReturnValue(userAgent);
  vi.spyOn(window.navigator, 'maxTouchPoints', 'get').mockReturnValue(maxTouchPoints);
}

describe('getSubscribeTargets', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('gives an iPadOS device the Apple-mobile targets but not the macOS-only ClashX', () => {
    // iPadOS 13+ Safari reports itself as "Macintosh" with a touch screen.
    setNavigator(IPAD_OS13_UA, 5);

    const titles = getSubscribeTargets('https://example.com/sub').map((t) => t.title);

    expect(titles).toContain('Shadowrocket');
    expect(titles).toContain('Surge');
    expect(titles).not.toContain('ClashX');
  });

  it('still offers ClashX on a real desktop Mac (no touch screen)', () => {
    setNavigator(MAC_UA, 0);

    const titles = getSubscribeTargets('https://example.com/sub').map((t) => t.title);

    expect(titles).toContain('ClashX');
    expect(titles).not.toContain('Shadowrocket');
  });
});
