import { describe, expect, it } from 'vitest';
import { getLegacyConfirmDefaultText } from './legacy-confirm';

describe('legacy confirm default text', () => {
  it('uses Ant Design zh-CN modal defaults', () => {
    expect(getLegacyConfirmDefaultText('zh-CN')).toEqual({
      okText: '确 定',
      cancelText: '取 消',
    });
  });

  it('uses Ant Design en-US modal defaults', () => {
    expect(getLegacyConfirmDefaultText('en-US')).toEqual({
      okText: 'OK',
      cancelText: 'Cancel',
    });
  });
});
