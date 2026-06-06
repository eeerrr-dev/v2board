import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import dayjs, { type Dayjs } from 'dayjs';
import { LegacyCalendarIcon } from './legacy-ant-icon';

interface LegacyDatePickerProps {
  onChange: (value: string | null) => void;
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

function formatDateTitle(date: Dayjs) {
  return `${date.year()}年${date.month() + 1}月${date.date()}日`;
}

function formatDateTime(date: Dayjs) {
  return date.format('YYYY-MM-DD HH:mm:ss');
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

export function LegacyDatePicker({ onChange, style }: LegacyDatePickerProps) {
  const rootRef = useRef<HTMLSpanElement | null>(null);
  const popupRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const [showTime, setShowTime] = useState(false);
  const [viewMonth, setViewMonth] = useState(() => dayjs().startOf('month'));
  const [selected, setSelected] = useState<Dayjs | null>(null);
  const [value, setValue] = useState('');
  const [popupStyle, setPopupStyle] = useState<CSSProperties>({});
  const today = dayjs();
  const activeDate = selected ?? today;
  const headerDate = showTime ? activeDate : viewMonth;

  useEffect(() => {
    if (!open) return;
    const rect = rootRef.current?.getBoundingClientRect();
    if (rect) {
      setPopupStyle({
        left: rect.left + window.scrollX,
        top: rect.bottom + window.scrollY,
      });
    }

    const close = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || popupRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [open]);

  const applyValue = (date: Dayjs | null) => {
    if (!date) {
      setSelected(null);
      setValue('');
      onChange(null);
      return;
    }
    setSelected(date);
    setValue(formatDateTime(date));
    setViewMonth(date.startOf('month'));
    onChange(date.format('X'));
  };

  const selectDate = (date: Dayjs) => {
    const next = selected
      ? date.hour(selected.hour()).minute(selected.minute()).second(selected.second())
      : date.hour(0).minute(0).second(0);
    applyValue(next);
  };

  const selectTime = (unit: 'hour' | 'minute' | 'second', nextValue: number) => {
    if (!selected) return;
    applyValue(selected.set(unit, nextValue));
  };

  const selectNow = () => {
    applyValue(dayjs());
  };

  const closeWithValue = () => {
    if (!selected) return;
    setOpen(false);
    setShowTime(false);
  };

  const openPicker = () => {
    setViewMonth((selected ?? dayjs()).startOf('month'));
    setOpen(true);
  };

  const rows = getCalendarRows(viewMonth);

  useEffect(() => {
    if (!open) setShowTime(false);
  }, [open]);

  useEffect(() => {
    if (selected) {
      setViewMonth(selected.startOf('month'));
    } else {
      setShowTime(false);
    }
  }, [selected]);

  const popup = (
    <div
      ref={popupRef}
      className="ant-calendar-picker-container ant-calendar-picker-container-placement-bottomLeft"
      style={popupStyle}
    >
      <div className="ant-calendar ant-calendar-time" tabIndex={0}>
        <div className="ant-calendar-panel">
          <div className="ant-calendar-input-wrap">
            <div className="ant-calendar-date-input-wrap">
              <input
                className="ant-calendar-input "
                placeholder="请选择日期"
                value={value}
                onChange={() => undefined}
              />
            </div>
            <a role="button" title="清除" onClick={() => applyValue(null)}>
              <span className="ant-calendar-clear-btn" />
            </a>
          </div>
          <div tabIndex={0} className="ant-calendar-date-panel">
            <div className="ant-calendar-header">
              <div style={{ position: 'relative' }}>
                {showTime ? null : (
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
                    className={`ant-calendar-year-select${showTime ? ' ant-calendar-time-status' : ''}`}
                    role="button"
                    title={showTime ? undefined : '选择年份'}
                  >
                    {headerDate.year()}年
                  </a>
                  <a
                    className={`ant-calendar-month-select${showTime ? ' ant-calendar-time-status' : ''}`}
                    role="button"
                    title={showTime ? undefined : '选择月份'}
                  >
                    {headerDate.month() + 1}月
                  </a>
                  {showTime ? (
                    <a className="ant-calendar-day-select ant-calendar-time-status" role="button">
                      {activeDate.date()}日
                    </a>
                  ) : null}
                </span>
                {showTime ? null : (
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
            {showTime ? (
              <div className="ant-calendar-time-picker">
                <div className="ant-calendar-time-picker-panel">
                  <div className="ant-calendar-time-picker-column-3 ant-calendar-time-picker-inner">
                    <div className="ant-calendar-time-picker-input-wrap">
                      <input
                        className="ant-calendar-time-picker-input"
                        placeholder="请选择时间"
                        value={selected ? selected.format('HH:mm:ss') : '00:00:00'}
                        onChange={() => undefined}
                      />
                    </div>
                    <div className="ant-calendar-time-picker-combobox">
                      <TimeColumn
                        max={23}
                        selected={selected?.hour() ?? 0}
                        onSelect={(next) => selectTime('hour', next)}
                      />
                      <TimeColumn
                        max={59}
                        selected={selected?.minute() ?? 0}
                        onSelect={(next) => selectTime('minute', next)}
                      />
                      <TimeColumn
                        max={59}
                        selected={selected?.second() ?? 0}
                        onSelect={(next) => selectTime('second', next)}
                      />
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
            <div className="ant-calendar-footer ant-calendar-footer-show-ok">
              <span className="ant-calendar-footer-btn">
                <a
                  className="ant-calendar-today-btn "
                  role="button"
                  title={formatDateTitle(today)}
                  onClick={selectNow}
                >
                  此刻
                </a>
                <a
                  className={`ant-calendar-time-picker-btn${selected ? '' : ' ant-calendar-time-picker-btn-disabled'}`}
                  role="button"
                  onClick={() => selected && setShowTime((value) => !value)}
                >
                  {showTime ? '选择日期' : '选择时间'}
                </a>
                <a
                  className={`ant-calendar-ok-btn${selected ? '' : ' ant-calendar-ok-btn-disabled'}`}
                  role="button"
                  onClick={closeWithValue}
                >
                  确 定
                </a>
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
        style={{ minWidth: 195, ...style }}
        onClick={openPicker}
      >
        <div>
          <input
            readOnly
            placeholder="请选择日期"
            className="ant-calendar-picker-input ant-input"
            value={value}
          />
          <LegacyCalendarIcon className="ant-calendar-picker-icon" />
        </div>
      </span>
      {portal}
    </>
  );
}
