import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { LegacyInput } from './legacy-input';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('LegacyInput', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('renders the old Ant Design input shell without Ant Design 5 runtime classes', () => {
    const html = renderToStaticMarkup(
      <LegacyInput
        placeholder="输入任意关键字搜索"
        className="ant-input ml-2"
        style={{ width: 200 }}
      />,
    );

    expect(html).toContain('placeholder="输入任意关键字搜索"');
    expect(html).toContain('class="ant-input ml-2"');
    expect(html).toContain('type="text"');
    expect(html).toContain('value=""');
    expect(html).not.toContain('css-dev-only-do-not-override');
    expect(html).not.toContain('ant-input-outlined');
    expect(html).not.toContain('ant-input-css-var');
  });

  it('keeps the old runtime input attribute order after mount', async () => {
    await act(async () => {
      root.render(
        <LegacyInput
          placeholder="输入任意关键字搜索"
          className="ant-input ml-2"
          style={{ width: 200 }}
        />,
      );
    });

    expect(container.querySelector('input')?.outerHTML).toBe(
      '<input placeholder="输入任意关键字搜索" class="ant-input ml-2" type="text" value="" style="width: 200px;">',
    );
  });
});
