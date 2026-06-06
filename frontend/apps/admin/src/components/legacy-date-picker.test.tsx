import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs from 'dayjs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyDatePicker } from './legacy-date-picker';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('LegacyDatePicker', () => {
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
    document.querySelectorAll('.ant-calendar-picker-container').forEach((element) => {
      element.remove();
    });
  });

  it('renders the old Ant Design date picker input shell', () => {
    const html = renderToStaticMarkup(
      <LegacyDatePicker showTime style={{ width: '100%' }} onChange={() => undefined} />,
    );

    expect(html).toContain(
      '<span class="ant-calendar-picker" style="min-width:195px;width:100%"><div><input readOnly="" placeholder="请选择日期" class="ant-calendar-picker-input ant-input" value=""/><i aria-label="图标: calendar" class="anticon anticon-calendar ant-calendar-picker-icon">',
    );
    expect(html).not.toContain('ant-picker');
    expect(html).not.toContain('css-dev-only');
  });

  it('opens the old Ant Design calendar-time popup shell instead of a native datetime input', async () => {
    await act(async () => {
      root.render(
        <LegacyDatePicker showTime style={{ width: '100%' }} onChange={() => undefined} />,
      );
    });

    await act(async () => {
      container
        .querySelector('.ant-calendar-picker-input')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const popup = document.querySelector('.ant-calendar-picker-container');
    expect(popup?.className).toBe(
      'ant-calendar-picker-container ant-calendar-picker-container-placement-bottomLeft',
    );
    expect(popup?.querySelector('.ant-calendar.ant-calendar-time')).not.toBeNull();
    expect(popup?.querySelector('.ant-calendar-input ')?.getAttribute('placeholder')).toBe(
      '请选择日期',
    );
    expect(popup?.querySelector('.ant-calendar-table')).not.toBeNull();
    expect(popup?.querySelectorAll('.ant-calendar-column-header-inner').length).toBe(7);
    expect(popup?.querySelector('.ant-calendar-time-picker-btn-disabled')?.textContent).toBe(
      '选择时间',
    );
    expect(popup?.querySelector('.ant-calendar-ok-btn-disabled')?.textContent).toBe('确 定');
    expect(popup?.querySelector('input[type="datetime-local"]')).toBeNull();
  });

  it('selects a date like the old showTime DatePicker and closes only after OK', async () => {
    const onChange = vi.fn();
    const target = dayjs().date(15).hour(0).minute(0).second(0).millisecond(0);

    await act(async () => {
      root.render(<LegacyDatePicker showTime style={{ width: '100%' }} onChange={onChange} />);
    });
    await act(async () => {
      container
        .querySelector('.ant-calendar-picker-input')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const dayCell = document.querySelector(
      `.ant-calendar-cell[title="${target.year()}年${target.month() + 1}月${target.date()}日"]`,
    );
    await act(async () => {
      dayCell?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onChange.mock.lastCall?.[0]?.format('X')).toBe(target.format('X'));
    expect(container.querySelector<HTMLInputElement>('.ant-calendar-picker-input')?.value).toBe(
      target.format('YYYY-MM-DD HH:mm:ss'),
    );
    expect(document.querySelector('.ant-calendar-picker-container')).not.toBeNull();
    expect(document.querySelector('.ant-calendar-time-picker-btn-disabled')).toBeNull();
    expect(document.querySelector('.ant-calendar-ok-btn-disabled')).toBeNull();

    await act(async () => {
      document
        .querySelector('.ant-calendar-time-picker-btn')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(document.querySelector('.ant-calendar-time-picker')).not.toBeNull();
    expect(document.querySelectorAll('.ant-calendar-time-picker-select').length).toBe(3);
    expect(
      Array.from(document.querySelectorAll('.ant-calendar-time-picker-select-option-selected')).map(
        (element) => element.textContent,
      ),
    ).toEqual(['00', '00', '00']);
    expect(document.querySelector('.ant-calendar-time-picker-btn')?.textContent).toBe('选择日期');

    await act(async () => {
      document
        .querySelector('.ant-calendar-ok-btn')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(document.querySelector('.ant-calendar-picker-container')).toBeNull();
  });

  it('renders date-only mode like the old user drawer picker', async () => {
    const onChange = vi.fn();
    const target = dayjs().date(15).hour(0).minute(0).second(0).millisecond(0);

    await act(async () => {
      root.render(
        <LegacyDatePicker
          placeholder="长期有效"
          defaultValue={target}
          style={{ width: '100%' }}
          onChange={onChange}
        />,
      );
    });

    expect(container.querySelector('.ant-calendar-picker')?.getAttribute('style')).toBe(
      'width: 100%;',
    );
    expect(container.querySelector<HTMLInputElement>('.ant-calendar-picker-input')?.value).toBe(
      target.format('YYYY-MM-DD'),
    );

    await act(async () => {
      container
        .querySelector('.ant-calendar-picker-input')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const popup = document.querySelector('.ant-calendar-picker-container');
    expect(popup?.querySelector('.ant-calendar-time')).toBeNull();
    expect(popup?.querySelector('.ant-calendar-time-picker-btn')).toBeNull();
    expect(popup?.querySelector('.ant-calendar-ok-btn')).toBeNull();
    expect(popup?.querySelector('.ant-calendar-input ')?.getAttribute('placeholder')).toBe(
      '长期有效',
    );

    await act(async () => {
      document
        .querySelector(`.ant-calendar-cell[title="${target.year()}年${target.month() + 1}月16日"]`)
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onChange.mock.lastCall?.[0]?.format('YYYY-MM-DD')).toBe(
      target.date(16).format('YYYY-MM-DD'),
    );
    expect(document.querySelector('.ant-calendar-picker-container')).toBeNull();
  });
});
