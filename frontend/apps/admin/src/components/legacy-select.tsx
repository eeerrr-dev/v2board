import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { LegacyDownIcon } from './legacy-ant-icon';
import { LegacyEmpty } from './legacy-empty';

export type LegacySelectValue = string | number | null;

export interface LegacySelectOption {
  value: LegacySelectValue;
  label: string;
}

interface LegacySelectProps {
  id?: string;
  value?: LegacySelectValue;
  options: LegacySelectOption[];
  placeholder?: string;
  required?: boolean;
  size?: 'small' | 'default' | 'large';
  dropdownMatchSelectWidth?: boolean;
  getPopupContainer?: (trigger: HTMLElement) => HTMLElement | null;
  className?: string;
  style?: CSSProperties;
  onChange: (value: LegacySelectValue) => void;
}

interface DropdownCoords {
  container: HTMLElement;
  placement: 'bottomLeft' | 'topLeft';
  left: number;
  top: number;
  width: number;
}

type LegacyTransitionStatus = 'enter' | 'entering' | 'entered' | 'leave' | 'leaving' | 'exited';

function classNames(...values: Array<string | false | undefined>) {
  return values.filter(Boolean).join(' ');
}

function useLegacyTransitionStatus(
  open: boolean,
  duration: number,
  holdMs: number,
): LegacyTransitionStatus {
  const [status, setStatus] = useState<LegacyTransitionStatus>(open ? 'entered' : 'exited');

  useEffect(() => {
    if (open) {
      setStatus((current) => (current === 'entered' ? current : 'enter'));
      return;
    }
    setStatus((current) => (current === 'exited' ? current : 'leave'));
    const timer = window.setTimeout(() => setStatus('exited'), duration);
    return () => window.clearTimeout(timer);
  }, [duration, open]);

  useEffect(() => {
    if (status !== 'enter') return;
    const timer = window.setTimeout(() => setStatus('entering'), holdMs);
    return () => window.clearTimeout(timer);
  }, [holdMs, status]);

  useEffect(() => {
    if (status !== 'entering') return;
    const timer = window.setTimeout(() => setStatus('entered'), Math.max(duration - holdMs, 0));
    return () => window.clearTimeout(timer);
  }, [duration, holdMs, status]);

  useEffect(() => {
    if (status !== 'leave') return;
    const timer = window.setTimeout(() => setStatus('leaving'), holdMs);
    return () => window.clearTimeout(timer);
  }, [holdMs, status]);

  return status;
}

function createLegacySelectAriaId() {
  let seed = new Date().getTime();
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (token) => {
    const value = ((seed + 16 * Math.random()) % 16) | 0;
    seed = Math.floor(seed / 16);
    return (token === 'x' ? value : (value & 0x7) | 0x8).toString(16);
  });
}

function estimateDropdownHeight(optionCount: number) {
  return Math.min(Math.max(optionCount, 1) * 32 + 8, 258);
}

export function LegacySelect({
  id,
  value,
  options,
  placeholder,
  required,
  size,
  dropdownMatchSelectWidth = true,
  getPopupContainer,
  className,
  style,
  onChange,
}: LegacySelectProps) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const selectionRef = useRef<HTMLDivElement | null>(null);
  const dropdownRef = useRef<HTMLDivElement | null>(null);
  const blurTimer = useRef<number | undefined>(undefined);
  const [ariaId, setAriaId] = useState('');
  const [open, setOpen] = useState(false);
  const [focused, setFocused] = useState(false);
  const [activeValue, setActiveValue] = useState<LegacySelectValue | null>(null);
  const [coords, setCoords] = useState<DropdownCoords | null>(null);
  const dropdownStatus = useLegacyTransitionStatus(open, 230, 30);
  const selected = options.find((item) => item.value === value);
  const selectedLabel = selected
    ? selected.label
    : value === '' || value == null
      ? null
      : String(value);
  const initialActiveValue = selected?.value ?? options[0]?.value ?? null;
  const isEmpty = options.length === 0;

  const slideClass =
    dropdownStatus === 'leave'
      ? 'slide-up-leave'
      : dropdownStatus === 'leaving'
        ? 'slide-up-leave slide-up-leave-active'
        : dropdownStatus === 'enter'
          ? 'slide-up-enter'
          : dropdownStatus === 'entering'
            ? 'slide-up-enter slide-up-enter-active'
            : '';

  const reposition = useCallback(() => {
    const trigger = rootRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const dropdownHeight =
      dropdownRef.current?.offsetHeight ?? estimateDropdownHeight(options.length);
    const bottomTop = rect.bottom + window.scrollY + 4;
    const topTop = rect.top + window.scrollY - dropdownHeight - 4;
    const shouldFlipToTop =
      rect.bottom + dropdownHeight + 4 > window.innerHeight && rect.top >= dropdownHeight + 4;
    const placement = shouldFlipToTop ? 'topLeft' : 'bottomLeft';

    setCoords({
      container: getPopupContainer?.(trigger) ?? document.body,
      placement,
      left: rect.left + window.scrollX,
      top: placement === 'topLeft' ? topTop : bottomTop,
      width: rect.width,
    });
  }, [getPopupContainer, options.length]);

  useEffect(() => {
    setAriaId(createLegacySelectAriaId());
    return () => window.clearTimeout(blurTimer.current);
  }, []);

  useEffect(() => {
    if (!open) return;
    setActiveValue(initialActiveValue);
    reposition();
    window.addEventListener('scroll', reposition, true);
    window.addEventListener('resize', reposition);
    const close = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || dropdownRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener('click', close);
    return () => {
      window.removeEventListener('scroll', reposition, true);
      window.removeEventListener('resize', reposition);
      document.removeEventListener('click', close);
    };
  }, [initialActiveValue, open, reposition]);

  useEffect(() => {
    if (!open) return;
    const raf = window.requestAnimationFrame(reposition);
    return () => window.cancelAnimationFrame(raf);
  }, [open, reposition]);

  const selectOption = (nextValue: LegacySelectValue) => {
    onChange(nextValue);
    setOpen(false);
  };

  const moveActiveOption = (direction: 1 | -1) => {
    if (!options.length) return;
    setActiveValue((currentValue) => {
      const currentIndex =
        currentValue !== null ? options.findIndex((item) => item.value === currentValue) : -1;
      const nextIndex =
        currentIndex >= 0
          ? (currentIndex + direction + options.length) % options.length
          : direction === 1
            ? 0
            : options.length - 1;
      return options[nextIndex]?.value ?? null;
    });
  };

  const toggleOpen = () => setOpen((current) => !current);

  const toggleFromArrow = (event: ReactMouseEvent<HTMLSpanElement>) => {
    event.preventDefault();
    event.stopPropagation();
    setOpen((current) => {
      if (!current) window.setTimeout(() => selectionRef.current?.focus(), 0);
      return !current;
    });
  };

  const dropdown =
    dropdownStatus !== 'exited' && coords
      ? createPortal(
          <div
            ref={dropdownRef}
            className={classNames(
              'ant-select-dropdown ant-select-dropdown--single',
              coords.placement === 'topLeft' && 'ant-select-dropdown-placement-topLeft',
              coords.placement === 'bottomLeft' && 'ant-select-dropdown-placement-bottomLeft',
              isEmpty && 'ant-select-dropdown--empty',
              slideClass,
            )}
            style={{
              left: coords.left,
              top: coords.top,
              [dropdownMatchSelectWidth ? 'width' : 'minWidth']: coords.width,
            }}
          >
            <div id={ariaId} style={{ overflow: 'auto', transform: 'translateZ(0)' }}>
              <ul
                role="listbox"
                tabIndex={0}
                className="ant-select-dropdown-menu ant-select-dropdown-menu-root ant-select-dropdown-menu-vertical"
              >
                {isEmpty ? (
                  <li
                    role="option"
                    aria-disabled="true"
                    unselectable="on"
                    style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
                    className="ant-select-dropdown-menu-item ant-select-dropdown-menu-item-disabled"
                  >
                    <LegacyEmpty />
                  </li>
                ) : (
                  options.map((item) => (
                    <li
                      key={item.value}
                      role="option"
                      aria-selected={value === item.value}
                      unselectable="on"
                      style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
                      className={classNames(
                        'ant-select-dropdown-menu-item',
                        activeValue === item.value && 'ant-select-dropdown-menu-item-active',
                        value === item.value && 'ant-select-dropdown-menu-item-selected',
                      )}
                      onMouseEnter={() => setActiveValue(item.value)}
                      onMouseLeave={() => setActiveValue(null)}
                      onClick={() => selectOption(item.value)}
                    >
                      {item.label}
                    </li>
                  ))
                )}
              </ul>
            </div>
          </div>,
          coords.container,
        )
      : null;

  return (
    <>
      <div
        id={id}
        ref={rootRef}
        className={classNames(
          size === 'large' && 'ant-select-lg',
          size === 'small' && 'ant-select-sm',
          className,
          'ant-select',
          open && 'ant-select-open',
          (open || focused) && 'ant-select-focused',
          'ant-select-enabled',
        )}
        style={style}
        onFocus={() => {
          window.clearTimeout(blurTimer.current);
          setFocused(true);
        }}
        onBlur={() => {
          window.clearTimeout(blurTimer.current);
          blurTimer.current = window.setTimeout(() => {
            setFocused(false);
            setOpen(false);
          }, 10);
        }}
      >
        <div
          ref={selectionRef}
          className={`ant-select-selection
            ant-select-selection--single`}
          aria-required={required}
          role="combobox"
          aria-autocomplete="list"
          aria-haspopup="true"
          aria-controls={ariaId}
          aria-expanded={open}
          tabIndex={0}
          onClick={toggleOpen}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              if (!open) {
                setOpen(true);
                return;
              }
              if (activeValue !== null) selectOption(activeValue);
              return;
            }
            if (event.key === 'ArrowDown') {
              event.preventDefault();
              if (!open) {
                setOpen(true);
                return;
              }
              moveActiveOption(1);
              return;
            }
            if (event.key === 'ArrowUp') {
              if (!open) return;
              event.preventDefault();
              moveActiveOption(-1);
              return;
            }
            if (event.key === ' ') {
              if (open) return;
              event.preventDefault();
              setOpen(true);
              return;
            }
            if (event.key === 'Escape' && open) {
              event.preventDefault();
              event.stopPropagation();
              setOpen(false);
            }
          }}
        >
          <div className="ant-select-selection__rendered">
            {placeholder ? (
              <div
                unselectable="on"
                className="ant-select-selection__placeholder"
                style={{
                  display: selectedLabel === null ? 'block' : 'none',
                  userSelect: 'none',
                  WebkitUserSelect: 'none',
                }}
                onMouseDown={(event) => event.preventDefault()}
              >
                {placeholder}
              </div>
            ) : null}
            {selectedLabel !== null ? (
              <div
                className="ant-select-selection-selected-value"
                title={selectedLabel}
                style={{ display: 'block', opacity: 1 }}
              >
                {selectedLabel}
              </div>
            ) : null}
          </div>
          <span
            className="ant-select-arrow"
            unselectable="on"
            style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
            onClick={toggleFromArrow}
          >
            <LegacyDownIcon className="ant-select-arrow-icon" />
          </span>
        </div>
      </div>
      {dropdown}
    </>
  );
}
