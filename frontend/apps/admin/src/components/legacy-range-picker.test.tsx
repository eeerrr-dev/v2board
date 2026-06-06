import { useState } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs, { type Dayjs } from 'dayjs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LegacyRangePicker } from './legacy-range-picker';

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

type RangeValue = [Dayjs | null, Dayjs | null];

describe('LegacyRangePicker', () => {
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

  it('renders the old Ant Design range picker input shell', () => {
    const html = renderToStaticMarkup(
      <LegacyRangePicker
        style={{ width: '100%' }}
        showTime={{ format: 'HH:mm' }}
        format="YYYY-MM-DD HH:mm"
        placeholder={['Start Time', 'End Time']}
        value={[null, null]}
        onChange={() => undefined}
      />,
    );

    expect(html).toContain('<span class="ant-calendar-picker" style="width:100%">');
    expect(html).toContain(
      '<span class="ant-calendar-range-picker ant-calendar-picker-input ant-input">',
    );
    expect(html).toContain(
      '<input readOnly="" placeholder="Start Time" class="ant-calendar-range-picker-input" value=""/>',
    );
    expect(html).toContain('<span class="ant-calendar-range-picker-separator"> ~ </span>');
    expect(html).toContain(
      '<input readOnly="" placeholder="End Time" class="ant-calendar-range-picker-input" value=""/>',
    );
    expect(html).toContain('class="anticon anticon-calendar ant-calendar-picker-icon"');
    expect(html).not.toContain('ant-picker');
    expect(html).not.toContain('css-dev-only');
  });

  it('opens the old Ant Design range calendar-time popup shell', async () => {
    await act(async () => {
      root.render(
        <LegacyRangePicker
          style={{ width: '100%' }}
          showTime={{ format: 'HH:mm' }}
          format="YYYY-MM-DD HH:mm"
          placeholder={['Start Time', 'End Time']}
          value={[null, null]}
          onChange={() => undefined}
        />,
      );
    });

    await act(async () => {
      container
        .querySelector('.ant-calendar-range-picker')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const popup = document.querySelector('.ant-calendar-picker-container');
    expect(popup?.className).toBe(
      'ant-calendar-picker-container ant-calendar-picker-container-placement-bottomLeft',
    );
    expect(
      popup?.querySelector('.ant-calendar.ant-calendar-range.ant-calendar-time'),
    ).not.toBeNull();
    expect(popup?.querySelector('.ant-calendar-range-left')).not.toBeNull();
    expect(popup?.querySelector('.ant-calendar-range-right')).not.toBeNull();
    expect(popup?.querySelectorAll('.ant-calendar-date-input-wrap')).toHaveLength(2);
    expect(popup?.querySelectorAll('.ant-calendar-column-header-inner')).toHaveLength(14);
    expect(popup?.querySelector('.ant-calendar-time-picker-btn-disabled')?.textContent).toBe(
      '选择时间',
    );
    expect(popup?.querySelector('.ant-calendar-ok-btn-disabled')?.textContent).toBe('确 定');
    expect(popup?.querySelector('input[type="datetime-local"]')).toBeNull();
    expect(popup?.outerHTML).not.toContain('ant-picker');
  });

  it('selects start and end values and confirms them like the old showTime RangePicker', async () => {
    const onChange = vi.fn();
    const onOk = vi.fn();
    const start = dayjs().date(15).hour(0).minute(0).second(0).millisecond(0);
    const end = start.add(1, 'month').date(16);

    function ControlledRangePicker() {
      const [value, setValue] = useState<RangeValue>([null, null]);
      return (
        <LegacyRangePicker
          style={{ width: '100%' }}
          showTime={{ format: 'HH:mm' }}
          format="YYYY-MM-DD HH:mm"
          placeholder={['Start Time', 'End Time']}
          value={value}
          onChange={(next) => {
            setValue(next);
            onChange(next);
          }}
          onOk={onOk}
        />
      );
    }

    await act(async () => {
      root.render(<ControlledRangePicker />);
    });
    await act(async () => {
      container
        .querySelector('.ant-calendar-range-picker')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    await act(async () => {
      document
        .querySelector(
          `.ant-calendar-range-left .ant-calendar-cell[title="${start.year()}年${start.month() + 1}月${start.date()}日"]`,
        )
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onChange.mock.lastCall?.[0][0]?.format('X')).toBe(start.format('X'));
    expect(onChange.mock.lastCall?.[0][1]).toBeNull();
    expect(
      container.querySelectorAll<HTMLInputElement>('.ant-calendar-range-picker-input')[0]?.value,
    ).toBe(start.format('YYYY-MM-DD HH:mm'));
    expect(document.querySelector('.ant-calendar-ok-btn-disabled')).not.toBeNull();

    await act(async () => {
      document
        .querySelector(
          `.ant-calendar-range-right .ant-calendar-cell[title="${end.year()}年${end.month() + 1}月${end.date()}日"]`,
        )
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onChange.mock.lastCall?.[0][0]?.format('X')).toBe(start.format('X'));
    expect(onChange.mock.lastCall?.[0][1]?.format('X')).toBe(end.format('X'));
    expect(
      container.querySelectorAll<HTMLInputElement>('.ant-calendar-range-picker-input')[1]?.value,
    ).toBe(end.format('YYYY-MM-DD HH:mm'));
    expect(document.querySelector('.ant-calendar-ok-btn-disabled')).toBeNull();

    await act(async () => {
      document
        .querySelector('.ant-calendar-ok-btn')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onOk.mock.lastCall?.[0][0]?.format('X')).toBe(start.format('X'));
    expect(onOk.mock.lastCall?.[0][1]?.format('X')).toBe(end.format('X'));
    expect(document.querySelector('.ant-calendar-picker-container')).toBeNull();
  });
});
