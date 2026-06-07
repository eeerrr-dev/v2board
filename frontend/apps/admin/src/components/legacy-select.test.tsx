import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { LegacySelect } from './legacy-select';

describe('LegacySelect', () => {
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

  it('renders the old single defaultValue fallback before a controlled value is chosen', () => {
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

    expect(html).toContain('class="ant-select-selection-selected-value"');
    expect(html).toContain('title="1"');
    expect(html).toContain('>1</div>');
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
});
