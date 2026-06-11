import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { LegacyRadio } from './legacy-radio';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyRadio', () => {
  it('renders the old Radio.Button group DOM', () => {
    const html = renderToStaticMarkup(
      <LegacyRadio.Group value={0}>
        <LegacyRadio.Button value={0}>已开启</LegacyRadio.Button>
        <LegacyRadio.Button value={1}>已关闭</LegacyRadio.Button>
      </LegacyRadio.Group>,
    );

    expect(html).toContain('class="ant-radio-group ant-radio-group-outline"');
    expect(html).toContain('class="ant-radio-button-wrapper ant-radio-button-wrapper-checked"');
    expect(html).toContain('class="ant-radio-button ant-radio-button-checked"');
    expect(html).toContain('type="radio"');
    expect(html).toContain('class="ant-radio-button-input"');
    expect(html).toContain('value="0"');
    expect(html).toContain('checked=""');
    expect(html).toContain('class="ant-radio-button-inner"');
  });

  it('keeps old Radio.Group classes, options, names, and disabled propagation', () => {
    const html = renderToStaticMarkup(
      <LegacyRadio.Group
        buttonStyle="solid"
        className="ticket-filter"
        defaultValue="open"
        name="ticket_status"
        options={[
          'open',
          { disabled: true, label: 'closed', value: 'closed' },
        ]}
        size="large"
      />,
    );

    expect(html).toContain(
      'class="ant-radio-group ant-radio-group-solid ant-radio-group-large ticket-filter"',
    );
    expect(html).toContain('name="ticket_status"');
    expect(html).toContain('class="ant-radio-wrapper ant-radio-wrapper-checked"');
    expect(html).toContain('class="ant-radio ant-radio-checked"');
    expect(html).toContain('class="ant-radio-wrapper ant-radio-wrapper-disabled"');
    expect(html).toContain('class="ant-radio ant-radio-disabled"');
    expect(html).toContain('disabled=""');
  });

  it('emits the old typed option value from group onChange', async () => {
    const onChange = vi.fn();
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(
        <LegacyRadio.Group defaultValue={0} onChange={onChange}>
          <LegacyRadio.Button value={0}>已开启</LegacyRadio.Button>
          <LegacyRadio.Button value={1}>已关闭</LegacyRadio.Button>
        </LegacyRadio.Group>,
      );
    });

    const inputs = container.querySelectorAll<HTMLInputElement>('input');

    await act(async () => {
      inputs[1]!.click();
    });

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]![0].target.value).toBe(1);
    expect(inputs[1]!.checked).toBe(true);

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
  });
});
