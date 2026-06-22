import {
  Children,
  isValidElement,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type FocusEvent as ReactFocusEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactElement,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import {
  LegacyCloseCircleIcon,
  LegacyCloseIcon,
  LegacyDownIcon,
  LegacyLoadingIcon,
} from './legacy-ant-icon';
import { LegacyEmpty } from './legacy-empty';

export type LegacySelectValue = string | number | null | undefined;
type LegacyMultipleSelectValue = Array<string | number>;
type LegacyMultipleSelectMode = 'multiple' | 'tags';
type LegacySelectMode = LegacyMultipleSelectMode | 'single';

export interface LegacySelectOption {
  value: LegacySelectValue;
  label: ReactNode;
  selectedLabel?: ReactNode;
  selectedTitle?: string;
  disabled?: boolean;
  groupKey?: string;
  groupLabel?: ReactNode;
  title?: string;
}

interface LegacySelectBaseProps {
  allowClear?: boolean;
  children?: ReactNode;
  id?: string;
  options?: LegacySelectOption[];
  placeholder?: string;
  required?: boolean;
  size?: 'small' | 'default' | 'large';
  disabled?: boolean;
  loading?: boolean;
  showArrow?: boolean;
  open?: boolean;
  defaultOpen?: boolean;
  dropdownMatchSelectWidth?: boolean;
  dropdownClassName?: string;
  dropdownStyle?: CSSProperties;
  dropdownMenuStyle?: CSSProperties;
  getPopupContainer?: (trigger: HTMLElement) => HTMLElement | null;
  notFoundContent?: ReactNode;
  tabIndex?: number;
  className?: string;
  style?: CSSProperties;
  onBlur?: (event: ReactFocusEvent<HTMLDivElement>) => void;
  onDropdownVisibleChange?: (open: boolean) => void;
  onFocus?: (event: ReactFocusEvent<HTMLDivElement>) => void;
  onSearch?: (value: string) => void;
}

interface LegacySelectSingleProps extends LegacySelectBaseProps {
  mode?: Extract<LegacySelectMode, 'single'>;
  defaultValue?: LegacySelectValue;
  value?: LegacySelectValue;
  onChange?: (value: LegacySelectValue, option?: LegacySelectOption) => void;
  onDeselect?: never;
  onSelect?: (value: LegacySelectValue, option?: LegacySelectOption) => void;
}

interface LegacySelectMultipleProps extends LegacySelectBaseProps {
  mode: LegacyMultipleSelectMode;
  defaultValue?: LegacyMultipleSelectValue;
  value?: LegacyMultipleSelectValue;
  onChange?: (value: LegacyMultipleSelectValue, options?: LegacySelectOption[]) => void;
  onDeselect?: (value: string | number, option?: LegacySelectOption) => void;
  onSelect?: (value: string | number, option?: LegacySelectOption) => void;
}

type LegacySelectProps = LegacySelectSingleProps | LegacySelectMultipleProps;

interface LegacySelectOptionProps {
  children?: ReactNode;
  disabled?: boolean;
  title?: string;
  value: Exclude<LegacySelectValue, null | undefined>;
}

interface LegacySelectOptGroupProps {
  children?: ReactNode;
  disabled?: boolean;
  label?: ReactNode;
}

interface DropdownCoords {
  container: HTMLElement;
  placement: 'bottomLeft' | 'topLeft';
  left: number;
  top: number;
  width: number;
}

type LegacyTransitionStatus = 'enter' | 'entering' | 'entered' | 'leave' | 'leaving' | 'exited';
const LEGACY_SELECT_OPEN_EVENT = 'v2board:legacy-select-open';

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

function getEnabledOptions(options: LegacySelectOption[]) {
  return options.filter((item) => !item.disabled);
}

function getLegacySelectedOptionLabel(options: LegacySelectOption[], value: string | number) {
  return options.find((item) => legacySelectValuesEqual(item.value, value))?.label ?? String(value);
}

function getLegacySelectTitle(option: Pick<LegacySelectOption, 'label' | 'title' | 'value'>) {
  if (option.title !== undefined) return option.title;
  if (typeof option.label === 'string' || typeof option.label === 'number') {
    return String(option.label);
  }
  return option.value === null || option.value === undefined ? undefined : String(option.value);
}

function LegacySelectOptionComponent(_props: LegacySelectOptionProps) {
  return null;
}

function LegacySelectOptGroup(_props: LegacySelectOptGroupProps) {
  return null;
}

function optionFromElement(
  child: ReactElement<LegacySelectOptionProps>,
  group?: { key: string; label: ReactNode; disabled?: boolean },
): LegacySelectOption {
  const label = child.props.children ?? String(child.props.value);
  return {
    value: child.props.value,
    label,
    disabled: group?.disabled || child.props.disabled,
    groupKey: group?.key,
    groupLabel: group?.label,
    title: child.props.title,
  };
}

function collectLegacySelectOptions(children: ReactNode) {
  const collected: LegacySelectOption[] = [];
  Children.forEach(children, (child, index) => {
    if (!isValidElement(child)) return;
    if (child.type === LegacySelectOptGroup) {
      const groupChild = child as ReactElement<LegacySelectOptGroupProps>;
      const groupLabel = groupChild.props.label;
      const groupKey = String(groupChild.key ?? groupLabel ?? index);
      Children.forEach(groupChild.props.children, (optionChild) => {
        if (!isValidElement<LegacySelectOptionProps>(optionChild)) return;
        if (optionChild.type !== LegacySelectOptionComponent) return;
        collected.push(
          optionFromElement(optionChild, {
            key: groupKey,
            label: groupLabel,
            disabled: groupChild.props.disabled,
          }),
        );
      });
      return;
    }
    if (child.type !== LegacySelectOptionComponent) return;
    collected.push(optionFromElement(child as ReactElement<LegacySelectOptionProps>));
  });
  return collected;
}

function LegacySelectComponent({
  allowClear = false,
  children,
  id,
  value,
  defaultValue,
  options: optionsProp,
  mode,
  placeholder,
  required,
  size,
  disabled = false,
  loading = false,
  showArrow,
  open: openProp,
  defaultOpen = false,
  dropdownMatchSelectWidth = true,
  dropdownClassName,
  dropdownStyle,
  dropdownMenuStyle,
  getPopupContainer,
  notFoundContent,
  tabIndex = 0,
  className,
  style,
  onBlur,
  onChange,
  onDeselect,
  onDropdownVisibleChange,
  onFocus,
  onSearch,
  onSelect,
}: LegacySelectProps) {
  const options = useMemo(
    () => optionsProp ?? collectLegacySelectOptions(children),
    [children, optionsProp],
  );
  const rootRef = useRef<HTMLDivElement | null>(null);
  const selectionRef = useRef<HTMLDivElement | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const searchValueRef = useRef('');
  const dropdownRef = useRef<HTMLDivElement | null>(null);
  const blurTimer = useRef<number | undefined>(undefined);
  const selectIdRef = useRef(Symbol('legacy-select'));
  const [ariaId, setAriaId] = useState('');
  const [openState, setOpenState] = useState(defaultOpen);
  const [forceHidden, setForceHidden] = useState(false);
  const [focused, setFocused] = useState(false);
  const [searchValue, setSearchValue] = useState('');
  const [activeValue, setActiveValue] = useState<LegacySelectValue | null>(null);
  const [coords, setCoords] = useState<DropdownCoords | null>(null);
  const [internalValue, setInternalValue] = useState<
    LegacySelectValue | LegacyMultipleSelectValue | undefined
  >(() => defaultValue);
  const multiple = mode === 'multiple' || mode === 'tags';
  const open = openProp ?? openState;
  const dropdownStatus = useLegacyTransitionStatus(open, 230, 30);
  const isControlled = value !== undefined;
  const effectiveValue = isControlled ? value : internalValue;
  const singleValue = multiple ? undefined : (effectiveValue as LegacySelectValue | undefined);
  const multipleValues = normalizeLegacyMultipleValue(
    multiple ? (effectiveValue as LegacyMultipleSelectValue | undefined) : undefined,
  );
  const selected = multiple
    ? undefined
    : options.find((item) => legacySelectValuesEqual(item.value, singleValue));
  const hasUnmatchedControlledSingleValue =
    !multiple &&
    isControlled &&
    selected === undefined &&
    singleValue !== null &&
    singleValue !== undefined;
  const hasUnmatchedNaNSingleValue =
    !multiple &&
    selected === undefined &&
    typeof singleValue === 'number' &&
    Number.isNaN(singleValue);
  const selectedLabel = selected
    ? (selected.selectedLabel ?? selected.label)
    : hasUnmatchedControlledSingleValue || hasUnmatchedNaNSingleValue
      ? String(singleValue)
      : null;
  const selectedTitle =
    selected !== undefined
      ? (selected.selectedTitle ?? getLegacySelectTitle(selected))
      : hasUnmatchedControlledSingleValue || hasUnmatchedNaNSingleValue
        ? String(singleValue)
        : undefined;
  const enabledOptions = getEnabledOptions(options);
  const initialActiveValue = selected?.value ?? enabledOptions[0]?.value ?? null;
  const isEmpty = options.length === 0;
  const hasValue = multiple
    ? multipleValues.length > 0
    : selected !== undefined || hasUnmatchedControlledSingleValue || hasUnmatchedNaNSingleValue;
  const showClear = allowClear && !disabled && (hasValue || searchValue.length > 0);

  const setSelectOpen = useCallback(
    (nextOpen: boolean) => {
      if (disabled && nextOpen) return;
      if (nextOpen) {
        setForceHidden(false);
        window.dispatchEvent(
          new CustomEvent(LEGACY_SELECT_OPEN_EVENT, { detail: selectIdRef.current }),
        );
      }
      if (open !== nextOpen) {
        onDropdownVisibleChange?.(nextOpen);
      }
      if (openProp === undefined) {
        setOpenState(nextOpen);
      }
    },
    [disabled, onDropdownVisibleChange, open, openProp],
  );

  useEffect(() => {
    const hideWhenSiblingOpens = (event: Event) => {
      if (!(event instanceof CustomEvent)) return;
      if (event.detail === selectIdRef.current) return;
      if (openProp !== undefined) return;
      if (!open && dropdownStatus === 'exited') return;
      setOpenState(false);
      setForceHidden(true);
    };
    window.addEventListener(LEGACY_SELECT_OPEN_EVENT, hideWhenSiblingOpens);
    return () => window.removeEventListener(LEGACY_SELECT_OPEN_EVENT, hideWhenSiblingOpens);
  }, [dropdownStatus, open, openProp]);

  const slideClass =
    dropdownStatus === 'leave'
      ? 'slide-up-leave'
      : dropdownStatus === 'leaving'
        ? 'slide-up-leave slide-up-leave-active'
        : dropdownStatus === 'enter'
          ? 'slide-up-appear'
          : dropdownStatus === 'entering'
            ? 'slide-up-appear slide-up-appear-active'
            : '';

  const reposition = useCallback(() => {
    const trigger = rootRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const bottomTop = rect.bottom + window.scrollY + 4;
    const placement = 'bottomLeft';

    setCoords({
      container: getPopupContainer?.(trigger) ?? document.body,
      placement,
      left: rect.left + window.scrollX,
      top: bottomTop,
      width: rect.width,
    });
  }, [getPopupContainer]);

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
      setSelectOpen(false);
    };
    document.addEventListener('click', close);
    return () => {
      window.removeEventListener('scroll', reposition, true);
      window.removeEventListener('resize', reposition);
      document.removeEventListener('click', close);
    };
  }, [initialActiveValue, open, reposition, setSelectOpen]);

  useEffect(() => {
    if (!open) return;
    const raf = window.requestAnimationFrame(reposition);
    return () => window.cancelAnimationFrame(raf);
  }, [open, reposition]);

  const getOptionByValue = (nextValue: LegacySelectValue) =>
    options.find((item) => legacySelectValuesEqual(item.value, nextValue));

  const getOptionsByValues = (nextValues: LegacyMultipleSelectValue) =>
    nextValues
      .map((item) => getOptionByValue(item))
      .filter((item): item is LegacySelectOption => Boolean(item));

  const selectOption = (nextValue: LegacySelectValue) => {
    const option = getOptionByValue(nextValue);
    if (option?.disabled || disabled) return;
    if (!isControlled) setInternalValue(nextValue);
    (onChange as ((value: LegacySelectValue, option?: LegacySelectOption) => void) | undefined)?.(
      nextValue,
      option,
    );
    (onSelect as ((value: LegacySelectValue, option?: LegacySelectOption) => void) | undefined)?.(
      nextValue,
      option,
    );
    setSelectOpen(false);
  };

  const selectMultipleOption = (nextValue: LegacySelectValue) => {
    if (nextValue === null || nextValue === undefined || disabled) return;
    const option = getOptionByValue(nextValue);
    if (option?.disabled) return;
    const selected = multipleValues.some((item) => legacySelectValuesEqual(item, nextValue));
    const nextValues = selected
      ? multipleValues.filter((item) => !legacySelectValuesEqual(item, nextValue))
      : [...multipleValues, nextValue];
    if (!isControlled) setInternalValue(nextValues);
    (onChange as
      | ((value: LegacyMultipleSelectValue, options?: LegacySelectOption[]) => void)
      | undefined)?.(nextValues, getOptionsByValues(nextValues));
    if (selected) {
      (onDeselect as ((value: string | number, option?: LegacySelectOption) => void) | undefined)?.(
        nextValue,
        option,
      );
    } else {
      (onSelect as ((value: string | number, option?: LegacySelectOption) => void) | undefined)?.(
        nextValue,
        option,
      );
    }
    searchValueRef.current = '';
    setSearchValue('');
  };

  const removeMultipleOption = (nextValue: string | number) => {
    if (disabled) return;
    const nextValues = multipleValues.filter((item) => !legacySelectValuesEqual(item, nextValue));
    if (!isControlled) setInternalValue(nextValues);
    (onChange as
      | ((value: LegacyMultipleSelectValue, options?: LegacySelectOption[]) => void)
      | undefined)?.(nextValues, getOptionsByValues(nextValues));
    (onDeselect as ((value: string | number, option?: LegacySelectOption) => void) | undefined)?.(
      nextValue,
      getOptionByValue(nextValue),
    );
  };

  const addTagValue = (nextValue: string) => {
    const trimmed = nextValue.trim();
    if (!trimmed) return;
    if (!multipleValues.some((item) => legacySelectValuesEqual(item, trimmed))) {
      const nextValues = [...multipleValues, trimmed];
      if (!isControlled) setInternalValue(nextValues);
      (onChange as
        | ((value: LegacyMultipleSelectValue, options?: LegacySelectOption[]) => void)
        | undefined)?.(nextValues, getOptionsByValues(nextValues));
      (onSelect as ((value: string | number, option?: LegacySelectOption) => void) | undefined)?.(
        trimmed,
        getOptionByValue(trimmed),
      );
    }
    searchValueRef.current = '';
    setSearchValue('');
  };

  const moveActiveOption = (direction: 1 | -1) => {
    const activeOptions = getEnabledOptions(options);
    if (!activeOptions.length) return;
    setActiveValue((currentValue) => {
      const currentIndex =
        currentValue !== null ? activeOptions.findIndex((item) => item.value === currentValue) : -1;
      const nextIndex =
        currentIndex >= 0
          ? (currentIndex + direction + activeOptions.length) % activeOptions.length
          : direction === 1
            ? 0
            : activeOptions.length - 1;
      return activeOptions[nextIndex]?.value ?? null;
    });
  };

  const toggleOpen = () => {
    if (disabled) return;
    setSelectOpen(!open);
    window.setTimeout(() => {
      if (multiple) searchInputRef.current?.focus();
    }, 0);
  };

  const toggleFromArrow = (event: ReactMouseEvent<HTMLSpanElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (disabled) return;
    if (!open) window.setTimeout(() => selectionRef.current?.focus(), 0);
    setSelectOpen(!open);
  };

  const renderDropdownOption = (item: LegacySelectOption) => {
    const itemSelected = multiple
      ? multipleValues.some((selectedValue) =>
          legacySelectValuesEqual(selectedValue, item.value),
        )
      : legacySelectValuesEqual(singleValue ?? null, item.value);
    const optionKey = item.value ?? 'RC_SELECT_EMPTY_VALUE_KEY';

    return (
      <li
        key={optionKey}
        role="option"
        aria-selected={itemSelected}
        aria-disabled={item.disabled ? 'true' : undefined}
        unselectable="on"
        style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
        title={getLegacySelectTitle(item)}
        className={classNames(
          'ant-select-dropdown-menu-item',
          !item.disabled &&
            activeValue === item.value &&
            'ant-select-dropdown-menu-item-active',
          itemSelected && 'ant-select-dropdown-menu-item-selected',
          item.disabled && 'ant-select-dropdown-menu-item-disabled',
        )}
        onMouseEnter={() => !item.disabled && setActiveValue(item.value)}
        onMouseLeave={() => !item.disabled && setActiveValue(null)}
        onClick={
          item.disabled
            ? undefined
            : () => (multiple ? selectMultipleOption(item.value) : selectOption(item.value))
        }
      >
        {item.label}
      </li>
    );
  };

  const renderDropdownOptions = () => {
    const nodes: ReactNode[] = [];
    for (let index = 0; index < options.length; index += 1) {
      const item = options[index]!;
      if (!item.groupKey) {
        nodes.push(renderDropdownOption(item));
        continue;
      }
      const groupKey = item.groupKey;
      const groupOptions: LegacySelectOption[] = [];
      while (index < options.length && options[index]?.groupKey === groupKey) {
        groupOptions.push(options[index]!);
        index += 1;
      }
      index -= 1;
      nodes.push(
        <li key={groupKey} className="ant-select-dropdown-menu-item-group">
          <div
            title={
              typeof item.groupLabel === 'string' || typeof item.groupLabel === 'number'
                ? String(item.groupLabel)
                : undefined
            }
            className="ant-select-dropdown-menu-item-group-title"
          >
            {item.groupLabel}
          </div>
          <ul className="ant-select-dropdown-menu-item-group-list">
            {groupOptions.map((groupOption) => renderDropdownOption(groupOption))}
          </ul>
        </li>,
      );
    }
    return nodes;
  };

  // rc-select has no destroyPopupOnHide: once opened, the dropdown stays mounted and closes by
  // gaining `ant-select-dropdown-hidden`. SelectTrigger's popupClassName fills rc-trigger's
  // middle slot as `[dropdownClassName] --single/multiple [--empty]`, before the placement class.
  const dropdown =
    coords
      ? createPortal(
          <div
            ref={dropdownRef}
            className={classNames(
              'ant-select-dropdown',
              dropdownClassName,
              multiple ? 'ant-select-dropdown--multiple' : 'ant-select-dropdown--single',
              isEmpty && 'ant-select-dropdown--empty',
              coords.placement === 'topLeft' && 'ant-select-dropdown-placement-topLeft',
              coords.placement === 'bottomLeft' && 'ant-select-dropdown-placement-bottomLeft',
              (forceHidden || dropdownStatus === 'exited') && 'ant-select-dropdown-hidden',
              slideClass,
            )}
            style={{
              ...dropdownStyle,
              left: coords.left,
              top: coords.top,
              [dropdownMatchSelectWidth ? 'width' : 'minWidth']: coords.width,
            }}
          >
            <div id={ariaId} style={{ overflow: 'auto', transform: 'translateZ(0)' }}>
              <ul
                role="listbox"
                tabIndex={0}
                style={dropdownMenuStyle}
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
                    {notFoundContent === undefined ? (
                      <LegacyEmpty context="select" />
                    ) : (
                      notFoundContent
                    )}
                  </li>
                ) : (
                  renderDropdownOptions()
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
      setSelectOpen(false);
    }
  };

  const clearSelection = (event: ReactMouseEvent<HTMLSpanElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (!showClear) return;
    searchValueRef.current = '';
    setSearchValue('');
    if (multiple) {
      if (!isControlled) setInternalValue([]);
      (onChange as
        | ((value: LegacyMultipleSelectValue, options?: LegacySelectOption[]) => void)
        | undefined)?.([], []);
    } else {
      if (!isControlled) setInternalValue(undefined);
      (onChange as
        | ((value: LegacySelectValue, option?: LegacySelectOption) => void)
        | undefined)?.(undefined, undefined);
    }
    setSelectOpen(false);
    window.setTimeout(() => selectionRef.current?.focus(), 0);
  };

  const renderClear = () =>
    showClear ? (
      <span
        key="clear"
        className="ant-select-selection__clear"
        unselectable="on"
        style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
        onMouseDown={(event) => event.preventDefault()}
        onClick={clearSelection}
      >
        <LegacyCloseCircleIcon className="ant-select-clear-icon" />
      </span>
    ) : null;

  const renderArrow = (isMultiple: boolean) => {
    const shouldShowArrow = loading || (showArrow ?? !isMultiple);
    if (!shouldShowArrow) return null;
    return (
      <span
        key="arrow"
        className="ant-select-arrow"
        unselectable="on"
        style={{ userSelect: 'none', WebkitUserSelect: 'none' }}
        onClick={toggleFromArrow}
      >
        {loading ? <LegacyLoadingIcon /> : <LegacyDownIcon className="ant-select-arrow-icon" />}
      </span>
    );
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
      tabIndex={disabled ? -1 : tabIndex}
      onClick={toggleOpen}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === 'ArrowDown' || event.key === ' ') {
          event.preventDefault();
          setSelectOpen(true);
          return;
        }
        if (event.key === 'Escape' && open) {
          event.preventDefault();
          event.stopPropagation();
          setSelectOpen(false);
        }
      }}
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
                title={
                  typeof label === 'string' || typeof label === 'number'
                    ? String(label)
                    : String(item)
                }
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
                onChange={(event) => {
                  searchValueRef.current = event.target.value;
                  setSearchValue(event.target.value);
                  onSearch?.(event.target.value);
                }}
                onFocus={() => setFocused(true)}
                onKeyDown={handleMultipleSearchKeyDown}
                autoComplete="off"
                disabled={disabled}
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
      {renderClear()}
      {renderArrow(true)}
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
      tabIndex={disabled ? -1 : tabIndex}
      onClick={toggleOpen}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          if (!open) {
            setSelectOpen(true);
            return;
          }
          if (activeValue !== null) selectOption(activeValue);
          return;
        }
        if (event.key === 'ArrowDown') {
          event.preventDefault();
          if (!open) {
            setSelectOpen(true);
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
          setSelectOpen(true);
          return;
        }
        if (event.key === 'Escape' && open) {
          event.preventDefault();
          event.stopPropagation();
          setSelectOpen(false);
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
            title={selectedTitle}
            style={{ display: 'block', opacity: 1 }}
          >
            {selectedLabel}
          </div>
        ) : null}
      </div>
      {renderClear()}
      {renderArrow(false)}
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
          disabled && 'ant-select-disabled',
          !disabled && 'ant-select-enabled',
          allowClear && 'ant-select-allow-clear',
          showArrow === true && 'ant-select-show-arrow',
          showArrow === false && 'ant-select-no-arrow',
          loading && 'ant-select-loading',
        )}
        style={style}
        onFocus={(event) => {
          if (disabled) {
            event.preventDefault();
            return;
          }
          window.clearTimeout(blurTimer.current);
          setFocused(true);
          onFocus?.(event);
        }}
        onBlur={(event) => {
          if (disabled) {
            event.preventDefault();
            return;
          }
          window.clearTimeout(blurTimer.current);
          blurTimer.current = window.setTimeout(() => {
            if (mode === 'tags' && searchValueRef.current) addTagValue(searchValueRef.current);
            setFocused(false);
            setSelectOpen(false);
            onBlur?.(event);
          }, 10);
        }}
      >
        {multiple ? renderMultipleSelection() : renderSingleSelection()}
      </div>
      {dropdown}
    </>
  );
}

export const LegacySelect = Object.assign(LegacySelectComponent, {
  Option: LegacySelectOptionComponent,
  OptGroup: LegacySelectOptGroup,
  SECRET_COMBOBOX_MODE_DO_NOT_USE: 'SECRET_COMBOBOX_MODE_DO_NOT_USE',
});
