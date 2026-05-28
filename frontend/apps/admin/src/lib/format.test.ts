import { describe, expect, it } from 'vitest';
import { formatBytes, formatMoney, formatTrafficGiB } from '@v2board/config/format';

describe('admin formatters', () => {
  it('formats traffic in GiB', () => {
    expect(formatTrafficGiB(0)).toBe('0.00 GB');
    expect(formatTrafficGiB(1024 ** 3)).toBe('1.00 GB');
  });
  it('formats raw bytes', () => {
    expect(formatBytes(1024)).toBe('1.00 KB');
  });
  it('formats money in cents', () => {
    expect(formatMoney(12345)).toBe('¥123.45');
  });
});
