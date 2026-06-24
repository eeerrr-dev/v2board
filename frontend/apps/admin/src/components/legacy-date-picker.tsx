import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
} from 'react';
import { createPortal } from 'react-dom';
import dayjs, { type Dayjs } from 'dayjs';
import { LegacyCalendarIcon, LegacyCloseCircleIcon } from './legacy-ant-icon';
import { LegacyRangePicker } from './legacy-range-picker';

interface LegacyDatePickerProps {
  allowClear?: boolean;
  defaultValue?: Dayjs | false | null;
  disabled?: boolean;
  format?: string | string[];
  onChange: (value: Dayjs | null, dateString: string) => void;
  onOk?: (value: Dayjs) => void;
  onOpenChange?: (open: boolean) => void;
  placeholder?: string;
  popupStyle?: CSSProperties;
  showTime?: boolean | { format?: string };
  style?: CSSProperties;
}

interface CalendarCell {
  date: Dayjs;
  inViewMonth: boolean;
}

const WEEKDAYS = [
  ['周一', '一'],
  ['周二', '二'],
  ['周三', '三'],
  ['周四', '四'],
  ['周五', '五'],
  ['周六', '六'],
  ['周日', '日'],
] as const;

type TimeUnit = 'hour' | 'minute' | 'second';

function formatDateTitle(date: Dayjs) {
  return `${date.year()}年${date.month() + 1}月${date.date()}日`;
}

function normalizeFormat(format: LegacyDatePickerProps['format'], showTime: boolean) {
  const nextFormat = Array.isArray(format) ? format[0] : format;
  return nextFormat ?? (showTime ? 'YYYY-MM-DD HH:mm:ss' : 'YYYY-MM-DD');
}

function formatDisplayValue(date: Dayjs, format: string) {
  return date.format(format);
}

function normalizeDefaultValue(value: LegacyDatePickerProps['defaultValue']) {
  if (!value) return null;
  return value.isValid() ? value : null;
}

function getCalendarRows(viewMonth: Dayjs): CalendarCell[][] {
  const firstDay = viewMonth.startOf('month');
  const mondayOffset = (firstDay.day() + 6) % 7;
  const start = firstDay.subtract(mondayOffset, 'day');

  return Array.from({ length: 6 }, (_, row) =>
    Array.from({ length: 7 }, (_, column) => {
      const date = start.add(row * 7 + column, 'day');
      return { date, inViewMonth: date.month() === viewMonth.month() };
    }),
  );
}

function pad(value: number) {
  return String(value).padStart(2, '0');
}

function getTimeColumns(format: string): Array<{ max: number; unit: TimeUnit }> {
  const columns: Array<{ max: number; unit: TimeUnit }> = [];
  if (/[HhKk]/.test(format)) columns.push({ max: 23, unit: 'hour' });
  if (/m/.test(format)) columns.push({ max: 59, unit: 'minute' });
  if (/s/.test(format)) columns.push({ max: 59, unit: 'second' });
  return columns.length ? columns : [{ max: 23, unit: 'hour' }];
}

function getTimeFormat(showTime: LegacyDatePickerProps['showTime']) {
  return typeof showTime === 'object' && showTime.format ? showTime.format : 'HH:mm:ss';
}

function formatEmptyTime(format: string) {
  return dayjs().hour(0).minute(0).second(0).millisecond(0).format(format);
}

function TimeColumn({
  max,
  selected,
  onSelect,
}: {
  max: number;
  selected: number;
  onSelect: (value: number) => void;
}) {
  return (
    <div className="ant-calendar-time-picker-select">
      <ul>
        {Array.from({ length: max + 1 }, (_, value) => (
          <li
            key={value}
            role="button"
            className={value === selected ? 'ant-calendar-time-picker-select-option-selected' : ''}
            tabIndex={0}
            onClick={() => onSelect(value)}
          >
            {pad(value)}
          </li>
        ))}
      </ul>
    </div>
  );
}

function LegacyDatePickerComponent({
  allowClear = true,
  defaultValue,
  disabled = false,
  format,
  onChange,
  onOk,
  onOpenChange,
  placeholder = '请选择日期',
  popupStyle: customPopupStyle,
  showTime = false,
  style,
}: LegacyDatePickerProps) {
  const rootRef = useRef<HTMLSpanElement | null>(null);
  const popupRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const [showTimeOpen, setShowTimeOpen] = useState(false);
  const [viewMonth, setViewMonth] = useState(() =>
    (normalizeDefaultValue(defaultValue) ?? dayjs()).startOf('month'),
  );
  const [selected, setSelected] = useState<Dayjs | null>(() => normalizeDefaultValue(defaultValue));
  const [popupPlacement, setPopupPlacement] = useState<'bottomLeft' | 'bottomRight'>('bottomLeft');
  const hasTime = Boolean(showTime);
  const timeFormat = getTimeFormat(showTime);
  const timeColumns = getTimeColumns(timeFormat);
  const pickerFormat = normalizeFormat(format, hasTime);
  const [value, setValue] = useState(() => {
    const initial = normalizeDefaultValue(defaultValue);
    return initial ? formatDisplayValue(initial, pickerFormat) : '';
  });
  const [popupPositionStyle, setPopupPositionStyle] = useState<CSSProperties>({});
  const today = dayjs();
  const activeDate = selected ?? today;
  const headerDate = showTimeOpen ? activeDate : viewMonth;
  const showClear = allowClear && !disabled && Boolean(selected);

  const setPickerOpen = (nextOpen: boolean) => {
    setOpen(nextOpen);
    onOpenChange?.(nextOpen);
  };

  useEffect(() => {
    if (!open) return;
    const rect = rootRef.current?.getBoundingClientRect();
    if (rect) {
      const popupWidth = popupRef.current?.getBoundingClientRect().width || 280;
      const isInDrawer = Boolean(
        rootRef.current?.closest('.ant-drawer, .ant-drawer-open, .v2board-filter-drawer'),
      );
      const nextPlacement =
        isInDrawer || rect.left + popupWidth > window.innerWidth
          ? 'bottomRight'
          : 'bottomLeft';
      setPopupPlacement(nextPlacement);
      setPopupPositionStyle({
        left:
          nextPlacement === 'bottomRight'
            ? Math.max(window.scrollX, rect.right + window.scrollX - popupWidth)
            : rect.left + window.scrollX,
        top: rect.bottom + window.scrollY,
      });
    }

    const close = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || popupRef.current?.contains(target)) return;
      setPickerOpen(false);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [open]);

  const applyValue = (date: Dayjs | null) => {
    if (!date) {
      setSelected(null);
      setValue('');
      onChange(null, '');
      return;
    }
    const nextValue = formatDisplayValue(date, pickerFormat);
    setSelected(date);
    setValue(nextValue);
    setViewMonth(date.startOf('month'));
    onChange(date, nextValue);
  };

  const selectDate = (date: Dayjs) => {
    const next = selected
      ? date.hour(selected.hour()).minute(selected.minute()).second(selected.second())
      : date.hour(0).minute(0).second(0);
    applyValue(next);
    if (!hasTime) {
      setPickerOpen(false);
    }
  };

  const selectTime = (unit: 'hour' | 'minute' | 'second', nextValue: number) => {
    if (!selected) return;
    applyValue(selected.set(unit, nextValue));
  };

  const selectNow = () => {
    applyValue(dayjs());
    if (!hasTime) {
      setPickerOpen(false);
    }
  };

  const clearSelection = (event: ReactMouseEvent<HTMLElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (!showClear) return;
    applyValue(null);
  };

  const closeWithValue = () => {
    if (!selected) return;
    onOk?.(selected);
    setPickerOpen(false);
    setShowTimeOpen(false);
  };

  const openPicker = () => {
    if (disabled) return;
    setViewMonth((selected ?? dayjs()).startOf('month'));
    setPickerOpen(true);
  };

  const rows = getCalendarRows(viewMonth);

  useEffect(() => {
    if (!open) setShowTimeOpen(false);
  }, [open]);

  useEffect(() => {
    if (selected) {
      setValue(formatDisplayValue(selected, pickerFormat));
      setViewMonth(selected.startOf('month'));
    } else {
      setValue('');
      setShowTimeOpen(false);
    }
  }, [pickerFormat, selected]);

  // rc-trigger: prefixCls + " " + popupClassName("") + " " + placementCls → double space.
  const popup = (
    <div
      ref={popupRef}
      className={`ant-calendar-picker-container  ant-calendar-picker-container-placement-${popupPlacement} slide-up-appear slide-up-appear-active`}
      style={{ ...popupPositionStyle, ...customPopupStyle }}
    >
      <div className={`ant-calendar${hasTime ? ' ant-calendar-time' : ''}`} tabIndex={0}>
        <div className="ant-calendar-panel">
          <div className="ant-calendar-input-wrap">
            <div className="ant-calendar-date-input-wrap">
              <input
                className="ant-calendar-input "
                placeholder={placeholder}
                value={value}
                onChange={() => undefined}
              />
            </div>
            {showClear ? (
              <a role="button" title="清除" onClick={() => applyValue(null)}>
                <span className="ant-calendar-clear-btn" />
              </a>
            ) : null}
          </div>
          <div tabIndex={0} className="ant-calendar-date-panel">
            <div className="ant-calendar-header">
              <div style={{ position: 'relative' }}>
                {showTimeOpen ? null : (
                  <>
                    <a
                      className="ant-calendar-prev-year-btn"
                      role="button"
                      title="上一年 (Control键加左方向键)"
                      onClick={() => setViewMonth((date) => date.subtract(1, 'year'))}
                    />
                    <a
                      className="ant-calendar-prev-month-btn"
                      role="button"
                      title="上个月 (翻页上键)"
                      onClick={() => setViewMonth((date) => date.subtract(1, 'month'))}
                    />
                  </>
                )}
                <span className="ant-calendar-ym-select">
                  <a
                    className={`ant-calendar-year-select${showTimeOpen ? ' ant-calendar-time-status' : ''}`}
                    role="button"
                    title={showTimeOpen ? undefined : '选择年份'}
                  >
                    {headerDate.year()}年
                  </a>
                  <a
                    className={`ant-calendar-month-select${showTimeOpen ? ' ant-calendar-time-status' : ''}`}
                    role="button"
                    title={showTimeOpen ? undefined : '选择月份'}
                  >
                    {headerDate.month() + 1}月
                  </a>
                  {showTimeOpen ? (
                    <a className="ant-calendar-day-select ant-calendar-time-status" role="button">
                      {activeDate.date()}日
                    </a>
                  ) : null}
                </span>
                {showTimeOpen ? null : (
                  <>
                    <a
                      className="ant-calendar-next-month-btn"
                      title="下个月 (翻页下键)"
                      onClick={() => setViewMonth((date) => date.add(1, 'month'))}
                    />
                    <a
                      className="ant-calendar-next-year-btn"
                      title="下一年 (Control键加右方向键)"
                      onClick={() => setViewMonth((date) => date.add(1, 'year'))}
                    />
                  </>
                )}
              </div>
            </div>
            {showTimeOpen ? (
              <div className="ant-calendar-time-picker">
                <div className="ant-calendar-time-picker-panel">
                    <div className={`ant-calendar-time-picker-column-${timeColumns.length} ant-calendar-time-picker-inner`}>
                      <div className="ant-calendar-time-picker-input-wrap">
                        <input
                          className="ant-calendar-time-picker-input"
                          placeholder="请选择时间"
                          value={selected ? selected.format(timeFormat) : formatEmptyTime(timeFormat)}
                          onChange={() => undefined}
                        />
                      </div>
                      <div className="ant-calendar-time-picker-combobox">
                        {timeColumns.map(({ max, unit }) => (
                          <TimeColumn
                            key={unit}
                            max={max}
                            selected={selected?.get(unit) ?? 0}
                            onSelect={(next) => selectTime(unit, next)}
                          />
                        ))}
                      </div>
                    </div>
                  </div>
              </div>
            ) : null}
            <div className="ant-calendar-body">
              <table className="ant-calendar-table" cellSpacing={0} role="grid">
                <thead>
                  <tr role="row">
                    {WEEKDAYS.map(([title, label]) => (
                      <th
                        key={title}
                        role="columnheader"
                        title={title}
                        className="ant-calendar-column-header"
                      >
                        <span className="ant-calendar-column-header-inner">{label}</span>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody className="ant-calendar-tbody">
                  {rows.map((row) => {
                    const isCurrentWeek = row.some((cell) => cell.date.isSame(today, 'day'));
                    const isActiveWeek = row.some((cell) => cell.date.isSame(activeDate, 'day'));
                    const rowClassName = [
                      isCurrentWeek ? 'ant-calendar-current-week' : '',
                      isActiveWeek ? 'ant-calendar-active-week' : '',
                    ]
                      .filter(Boolean)
                      .join(' ');
                    return (
                      <tr
                        key={row[0]!.date.format('YYYY-MM-DD')}
                        role="row"
                        className={rowClassName}
                      >
                        {row.map((cell) => {
                          const isToday = cell.date.isSame(today, 'day');
                          const isActiveDay = cell.date.isSame(activeDate, 'day');
                          const isSelectedDate = Boolean(
                            selected && cell.date.isSame(selected, 'day'),
                          );
                          const isLastDay = cell.date.isSame(viewMonth.endOf('month'), 'day');
                          const cellClassName = [
                            'ant-calendar-cell',
                            !cell.inViewMonth
                              ? cell.date.isBefore(viewMonth, 'month')
                                ? 'ant-calendar-last-month-cell'
                                : 'ant-calendar-next-month-btn-day'
                              : '',
                            isToday ? 'ant-calendar-today' : '',
                            isLastDay ? 'ant-calendar-last-day-of-month' : '',
                            isSelectedDate ? 'ant-calendar-selected-date' : '',
                            isActiveDay ? 'ant-calendar-selected-day' : '',
                          ]
                            .filter(Boolean)
                            .join(' ');
                          return (
                            <td
                              key={cell.date.format('YYYY-MM-DD')}
                              role="gridcell"
                              title={formatDateTitle(cell.date)}
                              className={cellClassName}
                              onClick={() => selectDate(cell.date)}
                            >
                              <div
                                className="ant-calendar-date"
                                aria-selected={isActiveDay ? 'true' : 'false'}
                                aria-disabled="false"
                              >
                                {cell.date.date()}
                              </div>
                            </td>
                          );
                        })}
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <div className={`ant-calendar-footer${hasTime ? ' ant-calendar-footer-show-ok' : ''}`}>
              <span className="ant-calendar-footer-btn">
                <a
                  className="ant-calendar-today-btn "
                  role="button"
                  title={formatDateTitle(today)}
                  onClick={selectNow}
                >
                  {hasTime ? '此刻' : '今天'}
                </a>
                {hasTime ? (
                  <>
                    <a
                      className={`ant-calendar-time-picker-btn${selected ? '' : ' ant-calendar-time-picker-btn-disabled'}`}
                      role="button"
                      onClick={() => selected && setShowTimeOpen((value) => !value)}
                    >
                      {showTimeOpen ? '选择日期' : '选择时间'}
                    </a>
                    <a
                      className={`ant-calendar-ok-btn${selected ? '' : ' ant-calendar-ok-btn-disabled'}`}
                      role="button"
                      onClick={closeWithValue}
                    >
                      确 定
                    </a>
                  </>
                ) : null}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );

  const shouldRenderPopup = open && typeof document !== 'undefined';

  const portal = shouldRenderPopup ? createPortal(popup, document.body) : null;

  return (
    <>
      <span
        ref={rootRef}
        className="ant-calendar-picker"
        style={{ ...(hasTime ? { minWidth: 195 } : {}), ...style }}
        onClick={openPicker}
      >
        <div>
          <input
            readOnly
            disabled={disabled}
            placeholder={placeholder}
            className={`ant-calendar-picker-input ant-input${disabled ? ' ant-input-disabled' : ''}`}
            value={value}
          />
          {showClear ? (
            <LegacyCloseCircleIcon
              className="ant-calendar-picker-clear"
              onClick={clearSelection}
            />
          ) : null}
          <LegacyCalendarIcon className="ant-calendar-picker-icon" />
        </div>
      </span>
      {portal}
    </>
  );
}

export const LegacyDatePicker = Object.assign(LegacyDatePickerComponent, {
  RangePicker: LegacyRangePicker,
});
