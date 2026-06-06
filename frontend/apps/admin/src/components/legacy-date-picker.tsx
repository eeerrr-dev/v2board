import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import dayjs from 'dayjs';
import { LegacyCalendarIcon } from './legacy-ant-icon';

interface LegacyDatePickerProps {
  onChange: (value: string | null) => void;
  style?: CSSProperties;
}

export function LegacyDatePicker({ onChange, style }: LegacyDatePickerProps) {
  const rootRef = useRef<HTMLSpanElement | null>(null);
  const popupRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const [value, setValue] = useState('');
  const [popupStyle, setPopupStyle] = useState<CSSProperties>({});

  useEffect(() => {
    if (!open) return;
    const rect = rootRef.current?.getBoundingClientRect();
    if (rect) {
      setPopupStyle({
        left: rect.left + window.scrollX,
        top: rect.bottom + window.scrollY + 4,
        width: Math.max(rect.width, 195),
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

  const selectValue = (next: string) => {
    if (!next) {
      setValue('');
      onChange(null);
      return;
    }
    const date = dayjs(next);
    if (!date.isValid()) return;
    setValue(date.format('YYYY-MM-DD HH:mm:ss'));
    onChange(date.format('X'));
    setOpen(false);
  };

  return (
    <>
      <span
        ref={rootRef}
        className="ant-calendar-picker"
        style={{ minWidth: 195, ...style }}
        onClick={() => setOpen(true)}
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
      {open && typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={popupRef}
              className="ant-calendar-picker-container"
              style={{ position: 'absolute', zIndex: 1050, ...popupStyle }}
            >
              <input
                type="datetime-local"
                className="ant-input"
                style={{ width: '100%' }}
                onChange={(event) => selectValue(event.target.value)}
              />
            </div>,
            document.body,
          )
        : null}
    </>
  );
}
