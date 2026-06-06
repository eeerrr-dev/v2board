import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { LegacyCloseIcon, LegacyDownIcon } from './legacy-ant-icon';
import { LegacyEmpty } from './legacy-empty';

export type LegacySelectValue = string | number | null;
type LegacyMultipleSelectValue = Array<string | number>;
type LegacySelectMode = 'multiple' | 'tags';

export interface LegacySelectOption {
  value: LegacySelectValue;
  label: string;
}

interface LegacySelectBaseProps {
  id?: string;
  options: LegacySelectOption[];
  placeholder?: string;
  required?: boolean;
  size?: 'small' | 'default' | 'large';
  dropdownMatchSelectWidth?: boolean;
  getPopupContainer?: (trigger: HTMLElement) => HTMLElement | null;
  className?: string;
  style?: CSSProperties;
}

interface LegacySelectSingleProps extends LegacySelectBaseProps {
  mode?: undefined;
  value?: LegacySelectValue;
  onChange?: (value: LegacySelectValue) => void;
}

interface LegacySelectMultipleProps extends LegacySelectBaseProps {
  mode: LegacySelectMode;
  value?: LegacyMultipleSelectValue;
  onChange?: (value: LegacyMultipleSelectValue) => void;
}

type LegacySelectProps = LegacySelectSingleProps | LegacySelectMultipleProps;

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

function normalizeLegacyMultipleValue(
  value: LegacySelectValue | LegacyMultipleSelectValue | undefined,
): LegacyMultipleSelectValue {
  return Array.isArray(value)
    ? value.filter((item): item is string | number => item !== null && item !== undefined)
    : [];
}

function legacySelectValuesEqual(left: LegacySelectValue, right: LegacySelectValue) {
  return left === right;
}

function getLegacySelectedOptionLabel(options: LegacySelectOption[], value: string | number) {
  return options.find((item) => legacySelectValuesEqual(item.value, value))?.label ?? String(value);
}

export function LegacySelect({
  id,
  value,
  options,
  mode,
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
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const dropdownRef = useRef<HTMLDivElement | null>(null);
  const blurTimer = useRef<number | undefined>(undefined);
  const [ariaId, setAriaId] = useState('');
  const [open, setOpen] = useState(false);
  const [focused, setFocused] = useState(false);
  const [searchValue, setSearchValue] = useState('');
  const [activeValue, setActiveValue] = useState<LegacySelectValue | null>(null);
  const [coords, setCoords] = useState<DropdownCoords | null>(null);
  const dropdownStatus = useLegacyTransitionStatus(open, 230, 30);
  const multiple = mode === 'multiple' || mode === 'tags';
  const singleValue = multiple ? undefined : (value as LegacySelectValue | undefined);
  const multipleValues = normalizeLegacyMultipleValue(
    multiple ? (value as LegacyMultipleSelectValue | undefined) : undefined,
  );
  const selected = multiple ? undefined : options.find((item) => item.value === singleValue);
  const selectedLabel = selected
    ? selected.label
    : singleValue == null
      ? null
      : String(singleValue);
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
    (onChange as ((value: LegacySelectValue) => void) | undefined)?.(nextValue);
    setOpen(false);
  };

  const selectMultipleOption = (nextValue: LegacySelectValue) => {
    if (nextValue === null) return;
    const selected = multipleValues.some((item) => legacySelectValuesEqual(item, nextValue));
    (onChange as ((value: LegacyMultipleSelectValue) => void) | undefined)?.(
      selected
        ? multipleValues.filter((item) => !legacySelectValuesEqual(item, nextValue))
        : [...multipleValues, nextValue],
    );
    setSearchValue('');
  };

  const removeMultipleOption = (nextValue: string | number) => {
    (onChange as ((value: LegacyMultipleSelectValue) => void) | undefined)?.(
      multipleValues.filter((item) => !legacySelectValuesEqual(item, nextValue)),
    );
  };

  const addTagValue = (nextValue: string) => {
    const trimmed = nextValue.trim();
    if (!trimmed) return;
    if (!multipleValues.some((item) => legacySelectValuesEqual(item, trimmed))) {
      (onChange as ((value: LegacyMultipleSelectValue) => void) | undefined)?.([
        ...multipleValues,
        trimmed,
      ]);
    }
    setSearchValue('');
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

  const toggleOpen = () => {
    setOpen((current) => !current);
    window.setTimeout(() => {
      if (multiple) searchInputRef.current?.focus();
    }, 0);
  };

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
              multiple
                ? 'ant-select-dropdown ant-select-dropdown--multiple'
                : 'ant-select-dropdown ant-select-dropdown--single',
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
                  options.map((item) => {
                    const itemSelected = multiple
                      ? multipleValues.some((selectedValue) =>
                          legacySelectValuesEqual(selectedValue, item.value),
                        )
                      : legacySelectValuesEqual(singleValue ?? null, item.value);

                    return (
                      <li
                        key={item.value}
                        role="option"
                        aria-selected={itemSelected}
                        unselectable="on"
                        style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
                        className={classNames(
                          'ant-select-dropdown-menu-item',
                          activeValue === item.value && 'ant-select-dropdown-menu-item-active',
                          itemSelected && 'ant-select-dropdown-menu-item-selected',
                        )}
                        onMouseEnter={() => setActiveValue(item.value)}
                        onMouseLeave={() => setActiveValue(null)}
                        onClick={() =>
                          multiple ? selectMultipleOption(item.value) : selectOption(item.value)
                        }
                      >
                        {item.label}
                      </li>
                    );
                  })
                )}
              </ul>
            </div>
          </div>,
          coords.container,
        )
      : null;

  const handleMultipleSearchKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      if (mode === 'tags') addTagValue(searchValue);
      return;
    }
    if (event.key === 'Backspace' && !searchValue && multipleValues.length) {
      removeMultipleOption(multipleValues[multipleValues.length - 1]!);
      return;
    }
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
    }
  };

  const renderMultipleSelection = () => (
    <div
      ref={selectionRef}
      className={`ant-select-selection
            ant-select-selection--multiple`}
      aria-required={required}
      role="combobox"
      aria-autocomplete="list"
      aria-haspopup="true"
      aria-controls={ariaId}
      aria-expanded={open}
      onClick={toggleOpen}
    >
      <div className="ant-select-selection__rendered">
        <ul>
          {multipleValues.map((item) => {
            const label = getLegacySelectedOptionLabel(options, item);
            return (
              <li
                key={item}
                unselectable="on"
                className="ant-select-selection__choice"
                title={label}
                style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
                onMouseDown={(event) => event.preventDefault()}
              >
                <div className="ant-select-selection__choice__content">{label}</div>
                <span
                  className="ant-select-selection__choice__remove"
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    removeMultipleOption(item);
                  }}
                >
                  <LegacyCloseIcon />
                </span>
              </li>
            );
          })}
          <li className="ant-select-search ant-select-search--inline">
            <div className="ant-select-search__field__wrap">
              <input
                ref={searchInputRef}
                value={searchValue}
                className="ant-select-search__field"
                style={{ width: searchValue ? `${searchValue.length + 0.75}em` : '0.75em' }}
                onChange={(event) => setSearchValue(event.target.value)}
                onFocus={() => setFocused(true)}
                onKeyDown={handleMultipleSearchKeyDown}
              />
              <span className="ant-select-search__field__mirror">{searchValue || '\u00a0'}</span>
            </div>
          </li>
        </ul>
        {placeholder ? (
          <div
            unselectable="on"
            className="ant-select-selection__placeholder"
            style={{
              display: multipleValues.length || searchValue ? 'none' : 'block',
              userSelect: 'none',
              WebkitUserSelect: 'none',
            }}
            onMouseDown={(event) => event.preventDefault()}
          >
            {placeholder}
          </div>
        ) : null}
      </div>
    </div>
  );

  const renderSingleSelection = () => (
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
  );

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
        {multiple ? renderMultipleSelection() : renderSingleSelection()}
      </div>
      {dropdown}
    </>
  );
}
