import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { LegacyCheckbox } from './legacy-checkbox';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyCheckbox', () => {
  it('exposes the old antd static checkbox marker', () => {
    expect(
      (LegacyCheckbox as typeof LegacyCheckbox & { __ANT_CHECKBOX?: boolean }).__ANT_CHECKBOX,
    ).toBe(true);
  });

  it('renders the old checkbox wrapper DOM', () => {
    const html = renderToStaticMarkup(
      <LegacyCheckbox checked indeterminate value="force">
        强制更新到用户
      </LegacyCheckbox>,
    );

    expect(html).toContain(
      'class="ant-checkbox-wrapper ant-checkbox-wrapper-checked"',
    );
    expect(html).toContain(
      'class="ant-checkbox ant-checkbox-checked ant-checkbox-indeterminate"',
    );
    expect(html).toContain('type="checkbox"');
    expect(html).toContain('class="ant-checkbox-input"');
    expect(html).toContain('value="force"');
    expect(html).toContain('checked=""');
    expect(html).toContain('class="ant-checkbox-inner"');
    expect(html).toContain('<span>强制更新到用户</span>');
  });

  it('supports the old Checkbox.Group options API', async () => {
    const onChange = vi.fn();
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(
        <LegacyCheckbox.Group
          className="server-groups"
          defaultValue={['a']}
          name="groups"
          onChange={onChange}
          options={[
            { label: 'A', value: 'a' },
            { label: 'B', value: 'b' },
          ]}
        />,
      );
    });

    expect(container.innerHTML).toContain('class="ant-checkbox-group server-groups"');
    expect(container.innerHTML).toContain(
      'class="ant-checkbox-group-item ant-checkbox-wrapper ant-checkbox-wrapper-checked"',
    );

    const inputs = container.querySelectorAll<HTMLInputElement>('input');

    await act(async () => {
      inputs[1]!.click();
    });

    expect(onChange).toHaveBeenCalledWith(['a', 'b']);
    expect(inputs[1]!.checked).toBe(true);

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
  });

  it('emits the old typed checkbox value from child onChange', async () => {
    const onChange = vi.fn();
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(
        <LegacyCheckbox value={1} onChange={onChange}>
          待回复
        </LegacyCheckbox>,
      );
    });

    await act(async () => {
      container.querySelector<HTMLInputElement>('input')!.click();
    });

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]![0].target.value).toBe(1);
    expect(onChange.mock.calls[0]![0].target.checked).toBe(true);

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
  });
});
