import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LegacySelect } from './legacy-select';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacySelect', () => {
  let container: HTMLDivElement | undefined;
  let root: Root | undefined;

  afterEach(() => {
    if (root) {
      act(() => root?.unmount());
      root = undefined;
    }
    container?.remove();
    container = undefined;
    document.body.innerHTML = '';
  });

  it('renders the old rc-select single placeholder markup', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        placeholder="请选择知识语言"
        style={{ width: '100%' }}
        options={[
          { value: 'en-US', label: 'English' },
          { value: 'zh-CN', label: '简体中文' },
        ]}
        onChange={vi.fn()}
      />,
    );

    expect(html).toContain('class="ant-select ant-select-enabled"');
    expect(html).toContain('ant-select-selection');
    expect(html).toContain('ant-select-selection--single');
    expect(html).toContain('role="combobox"');
    expect(html).toContain('aria-autocomplete="list"');
    expect(html).toContain('class="ant-select-selection__rendered"');
    expect(html).toContain('unselectable="on" class="ant-select-selection__placeholder"');
    expect(html).toContain('class="ant-select-selection__placeholder"');
    expect(html).toContain('请选择知识语言');
    expect(html).toContain('class="ant-select-arrow"');
    expect(html).toContain('class="anticon anticon-down ant-select-arrow-icon"');
    expect(html).not.toContain('ant-select-selector');
    expect(html).not.toContain('ant-select-selection-search');
  });

  it('renders the selected value like antd v3 rc-select', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        value="zh-CN"
        placeholder="请选择知识语言"
        options={[
          { value: 'en-US', label: 'English' },
          { value: 'zh-CN', label: '简体中文' },
        ]}
        onChange={vi.fn()}
      />,
    );

    expect(html).toContain('class="ant-select-selection-selected-value"');
    expect(html).toContain('title="简体中文"');
    expect(html).toContain('简体中文');
    expect(html).not.toContain('ant-select-selection-item');
  });

  it('supports the old Select.Option static children API', () => {
    const html = renderToStaticMarkup(
      <LegacySelect defaultValue="zh-CN" placeholder="请选择知识语言">
        <LegacySelect.Option value="en-US">English</LegacySelect.Option>
        <LegacySelect.Option value="zh-CN" title="简体中文">
          简体中文
        </LegacySelect.Option>
      </LegacySelect>,
    );

    expect(LegacySelect.SECRET_COMBOBOX_MODE_DO_NOT_USE).toBe(
      'SECRET_COMBOBOX_MODE_DO_NOT_USE',
    );
    expect(html).toContain('class="ant-select-selection-selected-value"');
    expect(html).toContain('title="简体中文"');
    expect(html).toContain('简体中文');
  });

  it('keeps legacy mode="single" rendering as the old single select', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        mode="single"
        value="2"
        placeholder="指定订阅"
        options={[
          { value: '1', label: 'Basic' },
          { value: '2', label: 'Pro' },
        ]}
      />,
    );

    expect(html).toContain('class="ant-select-selection-selected-value"');
    expect(html).toContain('ant-select-selection--single');
    expect(html).toContain('title="Pro"');
    expect(html).toContain('Pro');
    expect(html).not.toContain('ant-select-selection--multiple');
  });

  it('does not render an unmatched single defaultValue as a selected option', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        defaultValue={1}
        placeholder="请选择知识语言"
        options={[
          { value: 'en-US', label: 'English' },
          { value: 'zh-CN', label: '简体中文' },
        ]}
        onChange={vi.fn()}
      />,
    );

    expect(html).toContain('class="ant-select-selection__placeholder"');
    expect(html).toContain('请选择知识语言');
    expect(html).not.toContain('class="ant-select-selection-selected-value"');
    expect(html).not.toContain('title="1"');
    expect(html).not.toContain('>1</div>');
  });

  it('renders the old raw value for an unmatched controlled single value', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        value={1}
        placeholder="指定订阅"
        options={[{ value: '1', label: 'Pro' }]}
        onChange={vi.fn()}
      />,
    );

    expect(html).toContain('class="ant-select-selection-selected-value"');
    expect(html).toContain('title="1"');
    expect(html).toContain('>1</div>');
    expect(html).not.toContain('Pro');
  });

  it('renders the old raw NaN defaultValue for single selects', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        defaultValue={Number.NaN}
        placeholder="推荐返利类型"
        options={[{ value: 0, label: '跟随系统设置' }]}
        onChange={vi.fn()}
      />,
    );

    expect(html).toContain('class="ant-select-selection-selected-value"');
    expect(html).toContain('title="NaN"');
    expect(html).toContain('>NaN</div>');
    expect(html).not.toContain('跟随系统设置');
  });

  it('lets a controlled value override the old single defaultValue fallback', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        defaultValue={1}
        value="zh-CN"
        placeholder="请选择知识语言"
        options={[
          { value: 'en-US', label: 'English' },
          { value: 'zh-CN', label: '简体中文' },
        ]}
        onChange={vi.fn()}
      />,
    );

    expect(html).toContain('title="简体中文"');
    expect(html).toContain('简体中文');
    expect(html).not.toContain('title="1"');
  });

  it('does not replace an explicit null value with the old defaultValue fallback', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        defaultValue={1}
        value={null}
        placeholder="请选择知识语言"
        options={[
          { value: 'en-US', label: 'English' },
          { value: 'zh-CN', label: '简体中文' },
        ]}
      />,
    );

    expect(html).toContain('class="ant-select-selection__placeholder"');
    expect(html).toContain('请选择知识语言');
    expect(html).not.toContain('title="1"');
  });

  it('can be rendered with form-injected change handling', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        value=""
        options={[
          { value: '', label: '无' },
          { value: 7, label: '日本节点' },
        ]}
      />,
    );

    expect(html).toContain('class="ant-select-selection-selected-value"');
    expect(html).toContain('title="无"');
    expect(html).toContain('无');
  });

  it('renders the old rc-select multiple choice and search markup', () => {
    const html = renderToStaticMarkup(
      <LegacySelect
        mode="multiple"
        value={[7]}
        placeholder="请选择权限组"
        options={[
          { value: 7, label: 'Default Group' },
          { value: 8, label: 'VIP Group' },
        ]}
      />,
    );

    expect(html).toContain('ant-select-selection--multiple');
    expect(html).toContain('class="ant-select-selection__choice"');
    expect(html).toContain('class="ant-select-selection__choice__content"');
    expect(html).toContain('Default Group');
    expect(html).toContain('class="ant-select-selection__choice__remove"');
    expect(html).toContain('class="ant-select-search ant-select-search--inline"');
    expect(html).toContain('class="ant-select-search__field"');
    expect(html).not.toContain('ant-select-selector');
    expect(html).not.toContain('ant-select-selection-item');
  });

  it('renders tags mode with the old inline search placeholder', () => {
    const html = renderToStaticMarkup(
      <LegacySelect mode="tags" value={[]} placeholder="输入后回车添加标签" options={[]} />,
    );

    expect(html).toContain('ant-select-selection--multiple');
    expect(html).toContain('class="ant-select-selection__placeholder"');
    expect(html).toContain('输入后回车添加标签');
    expect(html).toContain('class="ant-select-search__field__wrap"');
  });

  it('renders the old small Empty inside an empty dropdown', async () => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(<LegacySelect placeholder="请选择" options={[]} />);
      await Promise.resolve();
    });

    await act(async () => {
      host
        .querySelector('.ant-select-selection')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
      await new Promise((resolve) => window.setTimeout(resolve, 50));
    });

    const empty = document.body.querySelector<HTMLElement>('.ant-empty');
    const dropdown = document.body.querySelector<HTMLElement>('.ant-select-dropdown');

    expect(dropdown?.className).toContain('slide-up-appear');
    expect(dropdown?.className).not.toContain('slide-up-enter');
    expect(dropdown?.className).toContain('ant-select-dropdown-placement-bottomLeft');
    expect(dropdown?.className).not.toContain('ant-select-dropdown-placement-topLeft');
    expect(empty?.className).toBe('ant-empty ant-empty-normal ant-empty-small');
    expect(empty?.querySelector('.ant-empty-description')?.textContent).toBe('暂无数据');
  });

  it('supports the old Select.OptGroup static children API in the dropdown', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect placeholder="请选择节点" onChange={onChange}>
          <LegacySelect.OptGroup label="亚洲">
            <LegacySelect.Option value="jp">日本</LegacySelect.Option>
            <LegacySelect.Option disabled value="hk">
              香港
            </LegacySelect.Option>
          </LegacySelect.OptGroup>
        </LegacySelect>,
      );
      await Promise.resolve();
    });

    await act(async () => {
      host
        .querySelector('.ant-select-selection')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    expect(document.body.querySelector('.ant-select-dropdown-menu-item-group-title')?.textContent).toBe(
      '亚洲',
    );
    expect(document.body.querySelector('.ant-select-dropdown-menu-item-group-list')).not.toBeNull();
    expect(
      Array.from(document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item')).map(
        (item) => item.textContent,
      ),
    ).toEqual(['日本', '香港']);

    const option = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item'),
    ).find((item) => item.textContent === '日本');

    await act(async () => {
      option?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange.mock.calls.at(-1)?.[0]).toBe('jp');
    expect(onChange.mock.calls.at(-1)?.[1]).toMatchObject({
      value: 'jp',
      label: '日本',
      groupLabel: '亚洲',
    });
  });

  it('updates the visible selected label for old uncontrolled single selects', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          defaultValue="en-US"
          options={[
            { value: 'en-US', label: 'English' },
            { value: 'zh-CN', label: '简体中文' },
          ]}
          onChange={onChange}
        />,
      );
      await Promise.resolve();
    });

    expect(host.querySelector('.ant-select-selection-selected-value')?.textContent).toBe('English');

    await act(async () => {
      host
        .querySelector('.ant-select-selection')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    const option = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item'),
    ).find((item) => item.textContent === '简体中文');

    await act(async () => {
      option?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith('zh-CN', {
      value: 'zh-CN',
      label: '简体中文',
    });
    expect(host.querySelector('.ant-select-selection-selected-value')?.textContent).toBe(
      '简体中文',
    );
  });

  it('updates old uncontrolled multiple selections locally', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          mode="multiple"
          defaultValue={[7]}
          options={[
            { value: 7, label: 'Default Group' },
            { value: 8, label: 'VIP Group' },
          ]}
          onChange={onChange}
        />,
      );
      await Promise.resolve();
    });

    expect(host.querySelector('.ant-select-selection__choice__content')?.textContent).toBe(
      'Default Group',
    );

    await act(async () => {
      host
        .querySelector('.ant-select-selection')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    const option = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item'),
    ).find((item) => item.textContent === 'VIP Group');

    await act(async () => {
      option?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith(
      [7, 8],
      [
        { value: 7, label: 'Default Group' },
        { value: 8, label: 'VIP Group' },
      ],
    );
    const selectedLabels = Array.from(
      host.querySelectorAll<HTMLElement>('.ant-select-selection__choice__content'),
    ).map((item) => item.textContent);
    expect(selectedLabels).toEqual(['Default Group', 'VIP Group']);
  });

  it('commits typed tags on blur like the old rc-select', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          mode="tags"
          defaultValue={[]}
          placeholder="输入后回车添加标签"
          options={[]}
          onChange={onChange}
        />,
      );
      await Promise.resolve();
    });

    const input = host.querySelector<HTMLInputElement>('.ant-select-search__field');
    expect(input).not.toBeNull();

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        input,
        'ops',
      );
      input!.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      host
        .querySelector('.ant-select')
        ?.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
      await new Promise((resolve) => window.setTimeout(resolve, 12));
    });

    expect(onChange).toHaveBeenLastCalledWith(['ops'], []);
    expect(host.querySelector('.ant-select-selection__choice__content')?.textContent).toBe('ops');
  });

  it('renders and handles the old allowClear control for single selects', async () => {
    const onChange = vi.fn();
    const onDropdownVisibleChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          allowClear
          defaultValue="zh-CN"
          options={[
            { value: 'en-US', label: 'English' },
            { value: 'zh-CN', label: '简体中文' },
          ]}
          onChange={onChange}
          onDropdownVisibleChange={onDropdownVisibleChange}
        />,
      );
      await Promise.resolve();
    });

    expect(host.querySelector('.ant-select')?.className).toContain('ant-select-allow-clear');
    expect(host.querySelector('.ant-select-selection__clear')).not.toBeNull();
    expect(host.querySelector('.ant-select-clear-icon')?.className).toContain(
      'anticon-close-circle',
    );

    await act(async () => {
      host
        .querySelector('.ant-select-selection__clear')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith(undefined, undefined);
    expect(host.querySelector('.ant-select-selection-selected-value')).toBeNull();
    expect(document.body.querySelector('.ant-select-dropdown')).toBeNull();
    expect(onDropdownVisibleChange).not.toHaveBeenCalled();
  });

  it('clears old multiple selections with the rc-select clear callback shape', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          allowClear
          mode="multiple"
          defaultValue={[7]}
          options={[
            { value: 7, label: 'Default Group' },
            { value: 8, label: 'VIP Group' },
          ]}
          onChange={onChange}
        />,
      );
      await Promise.resolve();
    });

    await act(async () => {
      host
        .querySelector('.ant-select-selection__clear')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenLastCalledWith([], []);
    expect(host.querySelector('.ant-select-selection__choice')).toBeNull();
  });

  it('keeps disabled selects inert with the old disabled class and tabIndex', async () => {
    const onDropdownVisibleChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          disabled
          value="zh-CN"
          options={[
            { value: 'en-US', label: 'English' },
            { value: 'zh-CN', label: '简体中文' },
          ]}
          onDropdownVisibleChange={onDropdownVisibleChange}
        />,
      );
      await Promise.resolve();
    });

    const select = host.querySelector('.ant-select');
    const selection = host.querySelector<HTMLElement>('.ant-select-selection');
    expect(select?.className).toContain('ant-select-disabled');
    expect(select?.className).not.toContain('ant-select-enabled');
    expect(selection?.tabIndex).toBe(-1);

    await act(async () => {
      selection?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    expect(document.body.querySelector('.ant-select-dropdown')).toBeNull();
    expect(onDropdownVisibleChange).not.toHaveBeenCalled();
  });

  it('marks disabled options like rc-select and does not select them', async () => {
    const onChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          defaultValue="en-US"
          options={[
            { value: 'en-US', label: 'English' },
            { value: 'zh-CN', label: '简体中文', disabled: true },
          ]}
          onChange={onChange}
        />,
      );
      await Promise.resolve();
    });

    await act(async () => {
      host
        .querySelector('.ant-select-selection')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    const disabledOption = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item'),
    ).find((item) => item.textContent === '简体中文');

    expect(disabledOption?.className).toContain('ant-select-dropdown-menu-item-disabled');
    expect(disabledOption?.getAttribute('aria-disabled')).toBe('true');

    await act(async () => {
      disabledOption?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).not.toHaveBeenCalled();
    expect(host.querySelector('.ant-select-selection-selected-value')?.textContent).toBe('English');
  });

  it('reports old dropdown visibility changes when the popup opens and closes', async () => {
    const onDropdownVisibleChange = vi.fn();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <LegacySelect
          defaultValue="en-US"
          options={[
            { value: 'en-US', label: 'English' },
            { value: 'zh-CN', label: '简体中文' },
          ]}
          onDropdownVisibleChange={onDropdownVisibleChange}
        />,
      );
      await Promise.resolve();
    });

    await act(async () => {
      host
        .querySelector('.ant-select-selection')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    expect(onDropdownVisibleChange).toHaveBeenLastCalledWith(true);

    const option = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item'),
    ).find((item) => item.textContent === '简体中文');

    await act(async () => {
      option?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onDropdownVisibleChange).toHaveBeenLastCalledWith(false);
  });

  it('hides a closed single dropdown immediately when another select opens', async () => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    const host = container;

    await act(async () => {
      root?.render(
        <>
          <LegacySelect
            defaultValue="tcp"
            options={[
              { value: 'tcp', label: 'TCP' },
              { value: 'ws', label: 'WebSocket' },
            ]}
          />
          <LegacySelect
            defaultValue={null}
            options={[
              { value: null, label: '无' },
              { value: 'xtls-rprx-vision', label: 'xtls-rprx-vision' },
            ]}
          />
        </>,
      );
      await Promise.resolve();
    });

    const selections = host.querySelectorAll<HTMLElement>('.ant-select-selection');

    await act(async () => {
      selections[0]?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    const firstDropdown = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown'),
    ).find((item) => item.textContent?.includes('WebSocket'));
    expect(firstDropdown?.className).not.toContain('ant-select-dropdown-hidden');

    const webSocketOption = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown-menu-item'),
    ).find((item) => item.textContent === 'WebSocket');

    await act(async () => {
      webSocketOption?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(firstDropdown?.className).not.toContain('ant-select-dropdown-hidden');

    await act(async () => {
      selections[1]?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await new Promise((resolve) => window.requestAnimationFrame(resolve));
    });

    expect(firstDropdown?.className).toContain('ant-select-dropdown-hidden');
    const visibleDropdowns = Array.from(
      document.body.querySelectorAll<HTMLElement>('.ant-select-dropdown'),
    ).filter((item) => !item.className.includes('ant-select-dropdown-hidden'));
    expect(visibleDropdowns).toHaveLength(1);
    expect(visibleDropdowns[0]?.textContent).toContain('xtls-rprx-vision');
  });
});
