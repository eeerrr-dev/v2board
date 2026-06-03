import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');

describe('admin legacy entrypoint', () => {
  it('initializes legacy settings and dark mode before rendering', () => {
    expect(mainSource).toContain('applyAdminLegacySettings();\napplyInitialDarkMode();');
  });

  it('does not wrap the app in React StrictMode, matching the bundled admin entry', () => {
    expect(mainSource).not.toContain('StrictMode');
  });

  it('does not install a storage-event auth sync listener absent from the bundled admin entry', () => {
    expect(mainSource).not.toContain('setupAuthSync');
    expect(mainSource).not.toContain("from './lib/auth'");
  });

  it('keeps the admin Ant Design locale fixed to zh_CN like the bundled admin app', () => {
    expect(mainSource).toContain("import zhCN from 'antd/locale/zh_CN';");
    expect(mainSource).toContain('locale={zhCN}');
    expect(mainSource).not.toContain("antd/locale/en_US");
  });
});
