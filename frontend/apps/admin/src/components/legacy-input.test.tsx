import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  LegacyCheckboxInput,
  LegacyInput,
  LegacyInputCompactGroup,
  LegacyInputGroup,
  LegacyTextArea,
} from './legacy-input';

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

  it('keeps the old runtime controlled input attribute order', async () => {
    await act(async () => {
      root.render(
        <LegacyInput
          placeholder="请输入"
          className="ant-input"
          value="default"
          onChange={() => undefined}
        />,
      );
    });

    expect(container.querySelector('input')?.outerHTML).toBe(
      '<input placeholder="请输入" type="text" class="ant-input" value="default">',
    );
  });

  it('renders the old runtime suffix input shell', async () => {
    await act(async () => {
      root.render(
        <LegacyInput
          className="ant-input"
          suffix="%"
          type="number"
          placeholder="在订单金额基础上附加手续费"
          defaultValue="1"
          onChange={() => undefined}
        />,
      );
    });

    expect(container.querySelector('.ant-input-affix-wrapper')?.outerHTML).toBe(
      '<span class="ant-input-affix-wrapper"><input placeholder="在订单金额基础上附加手续费" type="number" class="ant-input" value="1"><span class="ant-input-suffix">%</span></span>',
    );
  });

  it('renders old affix classes and clear behavior for prefix, suffix and allowClear', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyInput
          allowClear
          className="ant-input"
          defaultValue="88"
          onChange={onChange}
          prefix={<span className="prefix">¥</span>}
          size="large"
          suffix="元"
        />,
      );
    });

    const wrapper = container.querySelector<HTMLElement>('.ant-input-affix-wrapper')!;
    const input = container.querySelector<HTMLInputElement>('input')!;
    expect(wrapper.className).toBe(
      'ant-input-affix-wrapper ant-input-affix-wrapper-lg ant-input-affix-wrapper-input-with-clear-btn',
    );
    expect(wrapper.querySelector('.ant-input-prefix')?.outerHTML).toBe(
      '<span class="ant-input-prefix"><span class="prefix">¥</span></span>',
    );
    expect(input.outerHTML).toContain('class="ant-input ant-input-lg" value="88"');
    expect(wrapper.querySelector('.ant-input-clear-icon')?.getAttribute('role')).toBe('button');
    expect(wrapper.querySelector('.ant-input-suffix')?.textContent).toBe('元');

    await act(async () => {
      wrapper.querySelector<HTMLElement>('.ant-input-clear-icon')!.click();
    });

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]![0].target.value).toBe('');
    expect(input.value).toBe('');
    expect(document.activeElement).toBe(input);
  });

  it('keeps old onPressEnter callback ordering before onKeyDown', async () => {
    const calls: string[] = [];
    await act(async () => {
      root.render(
        <LegacyInput
          className="ant-input"
          onKeyDown={() => calls.push('down')}
          onPressEnter={() => calls.push('enter')}
        />,
      );
    });

    container
      .querySelector<HTMLInputElement>('input')!
      .dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter', keyCode: 13 }));

    expect(calls).toEqual(['enter', 'down']);
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

  it('renders the old controlled textarea shell', async () => {
    await act(async () => {
      root.render(
        <LegacyTextArea
          rows={5}
          placeholder="请输入"
          className="ant-input"
          value="textarea value"
          onChange={() => undefined}
        />,
      );
    });

    expect(container.querySelector('textarea')?.outerHTML).toBe(
      '<textarea rows="5" placeholder="请输入" class="ant-input">textarea value</textarea>',
    );
  });

  it('supports the old Input.TextArea static API', async () => {
    await act(async () => {
      root.render(
        <LegacyInput.TextArea
          rows={3}
          placeholder="请输入公告内容"
          className="ant-input"
          defaultValue="公告"
        />,
      );
    });

    expect(container.querySelector('textarea')?.outerHTML).toBe(
      '<textarea rows="3" placeholder="请输入公告内容" class="ant-input">公告</textarea>',
    );
  });

  it('renders the old textarea allowClear wrapper and clears through onChange', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <LegacyTextArea allowClear rows={2} defaultValue="说明" onChange={onChange} />,
      );
    });

    const wrapper = container.querySelector<HTMLElement>('.ant-input-affix-wrapper')!;
    const textarea = container.querySelector<HTMLTextAreaElement>('textarea')!;
    expect(wrapper.className).toBe(
      'ant-input-affix-wrapper ant-input-affix-wrapper-textarea-with-clear-btn',
    );
    expect(textarea.outerHTML).toBe('<textarea rows="2" class="ant-input">说明</textarea>');
    expect(wrapper.querySelector('.ant-input-textarea-clear-icon')?.getAttribute('role')).toBe(
      'button',
    );

    await act(async () => {
      wrapper.querySelector<HTMLElement>('.ant-input-textarea-clear-icon')!.click();
    });

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]![0].target.value).toBe('');
    expect(textarea.value).toBe('');
    expect(document.activeElement).toBe(textarea);
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

  it('renders the old runtime input group with before and after addons', async () => {
    await act(async () => {
      root.render(
        <LegacyInputGroup
          type="number"
          addonBefore={<span className="selector">类型</span>}
          addonAfter="%"
          placeholder="请输入值"
          value="8"
          onChange={() => undefined}
        />,
      );
    });

    expect(container.querySelector('.ant-input-group-wrapper')?.outerHTML).toBe(
      '<span class="ant-input-group-wrapper"><span class="ant-input-wrapper ant-input-group"><span class="ant-input-group-addon"><span class="selector">类型</span></span><input type="number" placeholder="请输入值" class="ant-input" value="8"><span class="ant-input-group-addon">%</span></span></span>',
    );
  });

  it('supports old Input addon props directly on LegacyInput', async () => {
    await act(async () => {
      root.render(
        <LegacyInput
          addonBefore="https://"
          addonAfter=".com"
          defaultValue="api"
          placeholder="域名"
          size="small"
        />,
      );
    });

    expect(container.querySelector('.ant-input-group-wrapper')?.outerHTML).toBe(
      '<span class="ant-input-group-wrapper ant-input-group-wrapper-sm"><span class="ant-input-wrapper ant-input-group"><span class="ant-input-group-addon">https://</span><input placeholder="域名" type="text" class="ant-input ant-input-sm" value="api"><span class="ant-input-group-addon">.com</span></span></span>',
    );
  });

  it('renders the old compact input group classes', async () => {
    await act(async () => {
      root.render(
        <LegacyInputCompactGroup>
          <LegacyInput className="ant-input" placeholder="账号" style={{ width: '45%' }} />
          <LegacyInput className="ant-input" placeholder="@" disabled style={{ width: '10%' }} />
        </LegacyInputCompactGroup>,
      );
    });

    expect(container.querySelector('.ant-input-group-compact')?.outerHTML).toBe(
      '<span class="ant-input-group ant-input-group-compact"><input placeholder="账号" type="text" class="ant-input" value="" style="width: 45%;"><input disabled="" placeholder="@" type="text" class="ant-input ant-input-disabled" value="" style="width: 10%;"></span>',
    );
  });

  it('supports old compact input group sizing and passthrough class', () => {
    const html = renderToStaticMarkup(
      <LegacyInputCompactGroup size="small" className="extra-compact">
        <LegacyInput className="ant-input" />
      </LegacyInputCompactGroup>,
    );

    expect(html).toContain(
      '<span class="ant-input-group ant-input-group-sm ant-input-group-compact extra-compact">',
    );
  });

  it('supports the old Input.Group static API with optional compact mode', async () => {
    await act(async () => {
      root.render(
        <>
          <LegacyInput.Group className="loose-group">
            <LegacyInput className="ant-input" placeholder="域" />
          </LegacyInput.Group>
          <LegacyInput.Group compact size="large" className="compact-group">
            <LegacyInput className="ant-input" placeholder="账号" style={{ width: '45%' }} />
            <LegacyInput className="ant-input" placeholder="@" disabled style={{ width: '10%' }} />
          </LegacyInput.Group>
        </>,
      );
    });

    const groups = container.querySelectorAll<HTMLElement>('.ant-input-group');
    expect(groups[0]?.outerHTML).toBe(
      '<span class="ant-input-group loose-group"><input placeholder="域" type="text" class="ant-input" value=""></span>',
    );
    expect(groups[1]?.outerHTML).toBe(
      '<span class="ant-input-group ant-input-group-lg ant-input-group-compact compact-group"><input placeholder="账号" type="text" class="ant-input" value="" style="width: 45%;"><input disabled="" placeholder="@" type="text" class="ant-input ant-input-disabled" value="" style="width: 10%;"></span>',
    );
  });

  it('keeps the old disabled input and textarea class names', async () => {
    await act(async () => {
      root.render(
        <>
          <LegacyInput className="ant-input" disabled placeholder="禁用输入" />
          <LegacyTextArea className="ant-input" disabled rows={2} placeholder="禁用文本" />
        </>,
      );
    });

    expect(container.querySelector('input')?.className).toBe('ant-input ant-input-disabled');
    expect(container.querySelector('textarea')?.className).toBe('ant-input ant-input-disabled');
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
