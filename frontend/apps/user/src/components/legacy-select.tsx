import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '@/lib/cn';
import { DownIcon } from '@/components/ant-icon';
import { LegacyEmpty } from '@/components/legacy-empty';
import { useTransitionStatus } from '@/lib/use-transition-status';

export type LegacySelectValue = string | number;

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
  // antd's css-animation holds the base class for 30ms before adding "-active";
  // leave then runs the 0.2s antSlideUpOut, finishing at 30 + 200 = 230ms.
  const dropdownStatus = useTransitionStatus(open, 230, 30);
  const selected = options.find((item) => item.value === value);
  // antd v3 renders the matched option's label as the trigger text, but falls back to the
  // raw value when no option matches (e.g. the pagination size changer, whose Select value
  // is a string while its options are numeric — they never match, so antd shows the bare
  // value). An empty value shows the placeholder instead.
  const selectedLabel = selected ? selected.label : value === '' || value == null ? null : String(value);
  const initialActiveValue = selected?.value ?? options[0]?.value ?? null;
  const isEmpty = options.length === 0;

  // antd v3 transitionName="slide-up": transient enter/leave lifecycle classes.
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

  useEffect(() => {
    if (!open || activeValue === null) return;
    const raf = window.requestAnimationFrame(() => {
      const menu = dropdownRef.current?.querySelector<HTMLElement>('.ant-select-dropdown-menu');
      const activeItem = dropdownRef.current?.querySelector<HTMLElement>(
        '.ant-select-dropdown-menu-item-active',
      );
      if (!menu || !activeItem) return;
      const itemTop = activeItem.offsetTop;
      const itemBottom = itemTop + activeItem.offsetHeight;
      if (itemTop < menu.scrollTop) {
        menu.scrollTop = itemTop;
      } else if (itemBottom > menu.scrollTop + menu.clientHeight) {
        menu.scrollTop = itemBottom - menu.clientHeight;
      }
    });
    return () => window.cancelAnimationFrame(raf);
  }, [activeValue, open]);

  const selectOption = (nextValue: LegacySelectValue) => {
    onChange(nextValue);
    setOpen(false);
  };

  const toggleOpen = () => {
    setOpen((value) => !value);
  };

  const toggleFromArrow = (event: ReactMouseEvent<HTMLSpanElement>) => {
    event.preventDefault();
    event.stopPropagation();
    setOpen((value) => {
      if (!value) window.setTimeout(() => selectionRef.current?.focus(), 0);
      return !value;
    });
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

  // rc-select has no destroyPopupOnHide: once opened, the dropdown stays mounted and closes by
  // gaining `ant-select-dropdown-hidden`. SelectTrigger's popupClassName fills rc-trigger's
  // middle slot as `--single [--empty]`, before the placement class.
  const dropdown =
    coords
      ? createPortal(
          <div
            ref={dropdownRef}
            className={cn(
              'ant-select-dropdown ant-select-dropdown--single',
              isEmpty && 'ant-select-dropdown--empty',
              coords.placement === 'topLeft' && 'ant-select-dropdown-placement-topLeft',
              coords.placement === 'bottomLeft' && 'ant-select-dropdown-placement-bottomLeft',
              dropdownStatus === 'exited' && 'ant-select-dropdown-hidden',
              slideClass,
            )}
            style={{
              left: coords.left,
              top: coords.top,
              [dropdownMatchSelectWidth ? 'width' : 'minWidth']: coords.width,
            }}
          >
            {/* rc-select's DropdownMenu wraps the menu in a scroll container carrying the aria id. */}
            <div
              id={ariaId}
              style={{ overflow: 'auto', transform: 'translateZ(0)' }}
              onMouseDown={(event) => event.preventDefault()}
            >
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
                    {/* antd Select defaults to ConfigProvider renderEmpty("Select"), not rc-select's "Not Found". */}
                    <LegacyEmpty size="small" />
                  </li>
                ) : (
                  options.map((item) => (
                    <li
                      key={item.value}
                      role="option"
                      aria-selected={value === item.value}
                      unselectable="on"
                      style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
                      className={cn(
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
        className={[
          size === 'large' && 'ant-select-lg',
          size === 'small' && 'ant-select-sm',
          className,
          'ant-select',
          open && 'ant-select-open',
          (open || focused) && 'ant-select-focused',
          'ant-select-enabled',
        ]
          .filter(Boolean)
          .join(' ')}
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
          className="ant-select-selection ant-select-selection--single"
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
                className="ant-select-selection__placeholder"
                style={{
                  display: selectedLabel === null ? 'block' : 'none',
                  userSelect: 'none',
                  WebkitUserSelect: 'none',
                }}
                unselectable="on"
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
            <DownIcon className="ant-select-arrow-icon" />
          </span>
        </div>
      </div>
      {dropdown}
    </>
  );
}

function estimateDropdownHeight(optionCount: number) {
  return Math.min(Math.max(optionCount, 1) * 32 + 8, 258);
}

function createLegacySelectAriaId() {
  let seed = new Date().getTime();
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (token) => {
    const value = (seed + 16 * Math.random()) % 16 | 0;
    seed = Math.floor(seed / 16);
    return (token === 'x' ? value : (value & 0x7) | 0x8).toString(16);
  });
}
