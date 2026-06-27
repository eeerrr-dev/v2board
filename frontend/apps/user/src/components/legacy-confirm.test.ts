import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { getLegacyConfirmDefaultText } from './legacy-confirm';

const source = readFileSync(`${process.cwd()}/src/components/legacy-confirm.tsx`, 'utf8');

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

  it('renders through shadcn alert-dialog primitives instead of the Ant modal replica', () => {
    expect(source).toContain("from '@/components/ui/alert-dialog'");
    expect(source).toContain("from '@/components/ui/button'");
    expect(source).toContain('v2board-confirm-dialog');
    expect(source).toContain('v2board-confirm-primary');
    expect(source).not.toContain("from '@/components/ui/dialog'");
    expect(source).not.toContain('AntBtn');
    expect(source).not.toContain('QuestionCircleIcon');
    expect(source).not.toContain('LegacyLoadingIcon');
    expect(source).not.toContain('ant-modal');
  });
});
