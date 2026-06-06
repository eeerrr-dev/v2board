import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import dayjs, { type Dayjs } from 'dayjs';
import { LegacyCalendarIcon } from './legacy-ant-icon';

type LegacyRangeValue = [Dayjs | null, Dayjs | null];

interface LegacyRangePickerProps {
  format?: string;
  onChange: (value: LegacyRangeValue) => void;
  onOk?: (value: LegacyRangeValue) => void;
  placeholder?: [string, string];
  showTime?: { format?: string };
  style?: CSSProperties;
  value: LegacyRangeValue;
}

interface CalendarCell {
  date: Dayjs;
  inViewMonth: boolean;
}

type RangeSide = 'start' | 'end';

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

function normalizeRange(value: LegacyRangeValue): LegacyRangeValue {
  return [value[0]?.isValid() ? value[0] : null, value[1]?.isValid() ? value[1] : null];
}

function displayValue(value: Dayjs | null, format: string) {
  return value ? value.format(format) : '';
}

function withTimeFrom(baseDate: Dayjs, current: Dayjs | null) {
  return baseDate
    .hour(current?.hour() ?? 0)
    .minute(current?.minute() ?? 0)
    .second(current?.second() ?? 0)
    .millisecond(0);
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

function CalendarPanel({
  activeSide,
  range,
  side,
  showTimeOpen,
  today,
  viewMonth,
  onChangeMonth,
  onSelectDate,
  onSelectTime,
  onToggleTime,
}: {
  activeSide: RangeSide;
  range: LegacyRangeValue;
  side: RangeSide;
  showTimeOpen: boolean;
  today: Dayjs;
  viewMonth: Dayjs;
  onChangeMonth: (value: Dayjs) => void;
  onSelectDate: (side: RangeSide, date: Dayjs) => void;
  onSelectTime: (side: RangeSide, unit: 'hour' | 'minute' | 'second', value: number) => void;
  onToggleTime: (side: RangeSide) => void;
}) {
  const selected = side === 'start' ? range[0] : range[1];
  const activeDate = selected ?? viewMonth;
  const rows = getCalendarRows(viewMonth);
  const [start, end] = range;
  const showPanelTime = showTimeOpen && activeSide === side;
  const panelClass = side === 'start' ? 'ant-calendar-range-left' : 'ant-calendar-range-right';

  return (
    <div className={`ant-calendar-range-part ${panelClass}`}>
      <div className="ant-calendar-header">
        <div style={{ position: 'relative' }}>
          {showPanelTime ? null : (
            <>
              <a
                className="ant-calendar-prev-year-btn"
                role="button"
                title="上一年 (Control键加左方向键)"
                onClick={() => onChangeMonth(viewMonth.subtract(1, 'year'))}
              />
              <a
                className="ant-calendar-prev-month-btn"
                role="button"
                title="上个月 (翻页上键)"
                onClick={() => onChangeMonth(viewMonth.subtract(1, 'month'))}
              />
            </>
          )}
          <span className="ant-calendar-ym-select">
            <a
              className={`ant-calendar-year-select${showPanelTime ? ' ant-calendar-time-status' : ''}`}
              role="button"
              title={showPanelTime ? undefined : '选择年份'}
            >
              {(showPanelTime ? activeDate : viewMonth).year()}年
            </a>
            <a
              className={`ant-calendar-month-select${showPanelTime ? ' ant-calendar-time-status' : ''}`}
              role="button"
              title={showPanelTime ? undefined : '选择月份'}
            >
              {(showPanelTime ? activeDate : viewMonth).month() + 1}月
            </a>
            {showPanelTime ? (
              <a className="ant-calendar-day-select ant-calendar-time-status" role="button">
                {activeDate.date()}日
              </a>
            ) : null}
          </span>
          {showPanelTime ? null : (
            <>
              <a
                className="ant-calendar-next-month-btn"
                title="下个月 (翻页下键)"
                onClick={() => onChangeMonth(viewMonth.add(1, 'month'))}
              />
              <a
                className="ant-calendar-next-year-btn"
                title="下一年 (Control键加右方向键)"
                onClick={() => onChangeMonth(viewMonth.add(1, 'year'))}
              />
            </>
          )}
        </div>
      </div>

      {showPanelTime ? (
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
                  onSelect={(value) => onSelectTime(side, 'hour', value)}
                />
                <TimeColumn
                  max={59}
                  selected={selected?.minute() ?? 0}
                  onSelect={(value) => onSelectTime(side, 'minute', value)}
                />
                <TimeColumn
                  max={59}
                  selected={selected?.second() ?? 0}
                  onSelect={(value) => onSelectTime(side, 'second', value)}
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
              const isActiveWeek = row.some(
                (cell) => selected && cell.date.isSame(selected, 'day'),
              );
              const rowClassName = [
                isCurrentWeek ? 'ant-calendar-current-week' : '',
                isActiveWeek ? 'ant-calendar-active-week' : '',
              ]
                .filter(Boolean)
                .join(' ');

              return (
                <tr key={row[0]!.date.format('YYYY-MM-DD')} role="row" className={rowClassName}>
                  {row.map((cell) => {
                    const isStart = Boolean(start && cell.date.isSame(start, 'day'));
                    const isEnd = Boolean(end && cell.date.isSame(end, 'day'));
                    const isSelected = Boolean(selected && cell.date.isSame(selected, 'day'));
                    const inRange = Boolean(
                      start &&
                      end &&
                      cell.date.isAfter(start, 'day') &&
                      cell.date.isBefore(end, 'day'),
                    );
                    const isToday = cell.date.isSame(today, 'day');
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
                      inRange ? 'ant-calendar-in-range-cell' : '',
                      isStart ? 'ant-calendar-selected-start-date' : '',
                      isEnd ? 'ant-calendar-selected-end-date' : '',
                      isSelected ? 'ant-calendar-selected-day ant-calendar-selected-date' : '',
                    ]
                      .filter(Boolean)
                      .join(' ');

                    return (
                      <td
                        key={cell.date.format('YYYY-MM-DD')}
                        role="gridcell"
                        title={formatDateTitle(cell.date)}
                        className={cellClassName}
                        onClick={() => onSelectDate(side, cell.date)}
                      >
                        <div
                          className="ant-calendar-date"
                          aria-selected={isSelected ? 'true' : 'false'}
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
      <div className="ant-calendar-time-picker-wrap">
        <a
          className={`ant-calendar-time-picker-btn${selected ? '' : ' ant-calendar-time-picker-btn-disabled'}`}
          role="button"
          onClick={() => selected && onToggleTime(side)}
        >
          {showPanelTime ? '选择日期' : '选择时间'}
        </a>
      </div>
    </div>
  );
}

export function LegacyRangePicker({
  format = 'YYYY-MM-DD HH:mm',
  onChange,
  onOk,
  placeholder = ['Start Time', 'End Time'],
  style,
  value,
}: LegacyRangePickerProps) {
  const rootRef = useRef<HTMLSpanElement | null>(null);
  const popupRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const [activeSide, setActiveSide] = useState<RangeSide>('start');
  const [showTimeSide, setShowTimeSide] = useState<RangeSide | null>(null);
  const [draftRange, setDraftRange] = useState<LegacyRangeValue>(() => normalizeRange(value));
  const [leftMonth, setLeftMonth] = useState(() => (value[0] ?? dayjs()).startOf('month'));
  const [popupStyle, setPopupStyle] = useState<CSSProperties>({});
  const range = open ? draftRange : normalizeRange(value);
  const [start, end] = range;
  const today = dayjs();
  const rightMonth = leftMonth.add(1, 'month');

  useEffect(() => {
    if (!open) {
      setDraftRange(normalizeRange(value));
      return;
    }
    const next = normalizeRange(value);
    setDraftRange(next);
    setLeftMonth((next[0] ?? next[1] ?? dayjs()).startOf('month'));
  }, [open, value]);

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
      setShowTimeSide(null);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [open]);

  const commitRange = (next: LegacyRangeValue) => {
    setDraftRange(next);
    onChange(next);
  };

  const openPicker = () => {
    const next = normalizeRange(value);
    setDraftRange(next);
    setActiveSide(next[0] && !next[1] ? 'end' : 'start');
    setLeftMonth((next[0] ?? next[1] ?? dayjs()).startOf('month'));
    setOpen(true);
  };

  const clearRange = () => {
    commitRange([null, null]);
    setActiveSide('start');
    setShowTimeSide(null);
  };

  const selectDate = (side: RangeSide, date: Dayjs) => {
    const current = side === 'start' ? draftRange[0] : draftRange[1];
    const nextDate = withTimeFrom(date, current);
    const next: LegacyRangeValue =
      side === 'start' ? [nextDate, draftRange[1]] : [draftRange[0], nextDate];
    commitRange(next);
    setActiveSide(side === 'start' ? 'end' : 'start');
    setShowTimeSide(null);
  };

  const selectTime = (side: RangeSide, unit: 'hour' | 'minute' | 'second', nextValue: number) => {
    const current = side === 'start' ? draftRange[0] : draftRange[1];
    if (!current) return;
    const nextDate = current.set(unit, nextValue);
    commitRange(side === 'start' ? [nextDate, draftRange[1]] : [draftRange[0], nextDate]);
  };

  const selectNow = () => {
    const now = dayjs();
    const next: LegacyRangeValue =
      activeSide === 'start' ? [now, draftRange[1]] : [draftRange[0], now];
    commitRange(next);
    setActiveSide(activeSide === 'start' ? 'end' : 'start');
  };

  const confirmRange = () => {
    onOk?.(draftRange);
    setOpen(false);
    setShowTimeSide(null);
  };

  const popup = (
    <div
      ref={popupRef}
      className="ant-calendar-picker-container ant-calendar-picker-container-placement-bottomLeft"
      style={popupStyle}
    >
      <div className="ant-calendar ant-calendar-range ant-calendar-time" tabIndex={0}>
        <div className="ant-calendar-panel">
          <div className="ant-calendar-date-panel">
            <div className="ant-calendar-input-wrap">
              <div className="ant-calendar-date-input-wrap">
                <input
                  className="ant-calendar-input "
                  placeholder={placeholder[0]}
                  value={displayValue(start, format)}
                  onChange={() => undefined}
                  onFocus={() => setActiveSide('start')}
                />
              </div>
              <div className="ant-calendar-date-input-wrap">
                <input
                  className="ant-calendar-input "
                  placeholder={placeholder[1]}
                  value={displayValue(end, format)}
                  onChange={() => undefined}
                  onFocus={() => setActiveSide('end')}
                />
              </div>
              <a role="button" title="清除" onClick={clearRange}>
                <span className="ant-calendar-clear-btn" />
              </a>
            </div>
            <CalendarPanel
              activeSide={activeSide}
              range={range}
              side="start"
              showTimeOpen={showTimeSide === 'start'}
              today={today}
              viewMonth={leftMonth}
              onChangeMonth={setLeftMonth}
              onSelectDate={selectDate}
              onSelectTime={selectTime}
              onToggleTime={(side) => setShowTimeSide(showTimeSide === side ? null : side)}
            />
            <span className="ant-calendar-range-middle"> ~ </span>
            <CalendarPanel
              activeSide={activeSide}
              range={range}
              side="end"
              showTimeOpen={showTimeSide === 'end'}
              today={today}
              viewMonth={rightMonth}
              onChangeMonth={(next) => setLeftMonth(next.subtract(1, 'month'))}
              onSelectDate={selectDate}
              onSelectTime={selectTime}
              onToggleTime={(side) => setShowTimeSide(showTimeSide === side ? null : side)}
            />
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
                  className={`ant-calendar-ok-btn${start && end ? '' : ' ant-calendar-ok-btn-disabled'}`}
                  role="button"
                  onClick={() => start && end && confirmRange()}
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

  const portal =
    open && typeof document !== 'undefined' ? createPortal(popup, document.body) : null;

  return (
    <>
      <span ref={rootRef} className="ant-calendar-picker" style={style} onClick={openPicker}>
        <span className="ant-calendar-range-picker ant-calendar-picker-input ant-input">
          <input
            readOnly
            placeholder={placeholder[0]}
            className="ant-calendar-range-picker-input"
            value={displayValue(start, format)}
          />
          <span className="ant-calendar-range-picker-separator"> ~ </span>
          <input
            readOnly
            placeholder={placeholder[1]}
            className="ant-calendar-range-picker-input"
            value={displayValue(end, format)}
          />
          <LegacyCalendarIcon className="ant-calendar-picker-icon" />
        </span>
      </span>
      {portal}
    </>
  );
}
