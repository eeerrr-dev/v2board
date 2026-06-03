import { describe, expect, it } from 'vitest';
import { formatBytes, formatDateTime, formatMoney } from '@v2board/config/format';

describe('formatters', () => {
  it('formats bytes', () => {
    expect(formatBytes(-1)).toBe(0);
    expect(formatBytes(0)).toBe('0.00 B');
    expect(formatBytes(1024)).toBe('1024.00 B');
    expect(formatBytes(1025)).toBe('1.00 KB');
    expect(formatBytes(1024 * 1024)).toBe('1024.00 KB');
    expect(formatBytes(1024 * 1024 + 1)).toBe('1.00 MB');
    expect(formatBytes(1024 * 1024 * 1024)).toBe('1024.00 MB');
    expect(formatBytes(2 * 1024 * 1024 * 1024)).toBe('2.00 GB');
  });
  it('formats money in cents', () => {
    expect(formatMoney(12345)).toBe('¥123.45');
    expect(formatMoney(50, '$')).toBe('$0.50');
  });
  it('formats date time', () => {
    expect(formatDateTime(null)).toBe('-');
    const t = Math.floor(new Date('2026-05-23T00:00:00Z').getTime() / 1000);
    const s = formatDateTime(t);
    expect(s).toMatch(/2026-05-2[23] \d{2}:\d{2}:\d{2}/);
  });
});
