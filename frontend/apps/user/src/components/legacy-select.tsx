import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { cn } from '@/lib/cn';

export interface LegacySelectOption {
  value: string;
  label: string;
}

interface LegacySelectProps {
  id?: string;
  value: string;
  options: LegacySelectOption[];
  placeholder?: string;
  required?: boolean;
  className?: string;
  style?: CSSProperties;
  onChange: (value: string) => void;
}

export function LegacySelect({
  id,
  value,
  options,
  placeholder,
  required,
  className,
  style,
  onChange,
}: LegacySelectProps) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const selected = options.find((item) => item.value === value);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, [open]);

  const selectOption = (nextValue: string) => {
    onChange(nextValue);
    setOpen(false);
  };

  return (
    <div
      ref={rootRef}
      className={cn(
        'ant-select ant-select-enabled',
        open && 'ant-select-open ant-select-focused',
        className,
      )}
      style={style}
    >
      <div
        id={id}
        className="ant-select-selection ant-select-selection--single"
        aria-required={required}
        role="combobox"
        tabIndex={0}
        onClick={() => setOpen((value) => !value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            setOpen((value) => !value);
          }
          if (event.key === 'Escape') setOpen(false);
        }}
      >
        <div className="ant-select-selection__rendered">
          {selected ? (
            <div className="ant-select-selection-selected-value">{selected.label}</div>
          ) : placeholder ? (
            <div className="ant-select-selection__placeholder">{placeholder}</div>
          ) : null}
        </div>
        <span className="ant-select-arrow" unselectable="on">
          <i className="fa fa-angle-down ant-select-arrow-icon" />
        </span>
      </div>
      {open ? (
        <div className="ant-select-dropdown ant-select-dropdown-placement-bottomLeft">
          <div>
            <ul className="ant-select-dropdown-menu ant-select-dropdown-menu-root ant-select-dropdown-menu-vertical">
              {options.map((item) => (
                <li
                  key={item.value}
                  className={cn(
                    'ant-select-dropdown-menu-item',
                    value === item.value && 'ant-select-dropdown-menu-item-selected',
                  )}
                  onClick={() => selectOption(item.value)}
                >
                  {item.label}
                </li>
              ))}
            </ul>
          </div>
        </div>
      ) : null}
    </div>
  );
}
