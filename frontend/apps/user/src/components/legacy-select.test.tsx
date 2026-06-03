import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacySelect } from './legacy-select';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ i18n: { language: 'en-US' } }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('LegacySelect rc-select behavior', () => {
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
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  function renderSelect() {
    act(() => {
      root.render(
        <LegacySelect
          value="low"
          placeholder="Please choose"
          options={[
            { value: 'low', label: 'Low' },
            { value: 'high', label: 'High' },
          ]}
          onChange={() => {}}
        />,
      );
    });
  }

  it('closes after the legacy 10ms outer blur delay', () => {
    vi.useFakeTimers();
    renderSelect();

    const selection = container.querySelector('.ant-select-selection') as HTMLElement;
    act(() => {
      selection.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
      selection.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(container.querySelector('.ant-select-open')).not.toBeNull();

    act(() => {
      selection.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
    });

    expect(container.querySelector('.ant-select-open')).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(10);
    });

    expect(container.querySelector('.ant-select-open')).toBeNull();
    expect(container.querySelector('.ant-select-focused')).toBeNull();
  });

  it('renders the placeholder with the legacy unselectable marker', () => {
    act(() => {
      root.render(
        <LegacySelect
          placeholder="Please choose"
          options={[{ value: 'low', label: 'Low' }]}
          onChange={() => {}}
        />,
      );
    });

    expect(
      container.querySelector('.ant-select-selection__placeholder')?.getAttribute('unselectable'),
    ).toBe('on');
  });

  it('removes slide-up enter classes after the rc-animate enter motion ends', () => {
    vi.useFakeTimers();
    renderSelect();

    const selection = container.querySelector('.ant-select-selection') as HTMLElement;
    act(() => {
      selection.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const dropdown = document.body.querySelector('.ant-select-dropdown') as HTMLElement;
    expect(dropdown.className).toContain('slide-up-enter');
    expect(dropdown.className).not.toContain('slide-up-enter-active');

    act(() => {
      vi.advanceTimersByTime(30);
    });

    expect(dropdown.className).toContain('slide-up-enter-active');

    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(dropdown.className).not.toContain('slide-up-enter');
    expect(dropdown.className).not.toContain('slide-up-enter-active');
  });
});
