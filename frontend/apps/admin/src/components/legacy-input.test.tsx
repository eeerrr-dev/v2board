import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { LegacyCheckboxInput, LegacyInput, LegacyInputGroup, LegacyTextArea } from './legacy-input';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

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
      '<input placeholder="输入任意关键字搜索" type="text" class="ant-input ml-2" value="" style="width: 200px;">',
    );
  });

  it('renders the old runtime textarea shell', async () => {
    await act(async () => {
      root.render(
        <LegacyTextArea rows={4} placeholder="请输入套餐描述，支持HTML" className="ant-input" />,
      );
    });

    expect(container.querySelector('textarea')?.outerHTML).toBe(
      '<textarea rows="4" placeholder="请输入套餐描述，支持HTML" class="ant-input"></textarea>',
    );
  });

  it('renders the old runtime input group addon shell', async () => {
    await act(async () => {
      root.render(
        <LegacyInputGroup
          addonAfter="GB"
          placeholder="请输入套餐流量"
          value={undefined}
          onChange={() => undefined}
        />,
      );
    });

    expect(container.querySelector('.ant-input-group-wrapper')?.outerHTML).toBe(
      '<span class="ant-input-group-wrapper"><span class="ant-input-wrapper ant-input-group"><input placeholder="请输入套餐流量" type="text" class="ant-input" value=""><span class="ant-input-group-addon">GB</span></span></span>',
    );
  });

  it('keeps the old runtime number input group attribute order', async () => {
    await act(async () => {
      root.render(
        <LegacyInputGroup
          type="number"
          addonAfter="GB"
          placeholder="请输入流量"
          value="0.00"
          onChange={() => undefined}
        />,
      );
    });

    expect(container.querySelector('.ant-input-group-wrapper')?.outerHTML).toBe(
      '<span class="ant-input-group-wrapper"><span class="ant-input-wrapper ant-input-group"><input type="number" placeholder="请输入流量" class="ant-input" value="0.00"><span class="ant-input-group-addon">GB</span></span></span>',
    );
  });

  it('renders the old large addon input classes from Ant Design 3', async () => {
    await act(async () => {
      root.render(
        <LegacyInputGroup
          type="number"
          size="large"
          addonAfter="秒"
          placeholder="请输入"
          defaultValue="60"
          onChange={() => undefined}
        />,
      );
    });

    expect(container.querySelector('.ant-input-group-wrapper')?.outerHTML).toBe(
      '<span class="ant-input-group-wrapper ant-input-group-wrapper-lg"><span class="ant-input-wrapper ant-input-group"><input type="number" placeholder="请输入" class="ant-input ant-input-lg" value="60"><span class="ant-input-group-addon">秒</span></span></span>',
    );
  });

  it('keeps the old runtime checkbox attribute order after mount', async () => {
    await act(async () => {
      root.render(<LegacyCheckboxInput className="ant-checkbox-input" value="" />);
    });

    expect(container.querySelector('input')?.outerHTML).toBe(
      '<input type="checkbox" class="ant-checkbox-input" value="">',
    );
  });
});
