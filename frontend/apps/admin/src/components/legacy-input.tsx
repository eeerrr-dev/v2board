import {
  forwardRef,
  useImperativeHandle,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ChangeEvent,
  type ChangeEventHandler,
  type CSSProperties,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type KeyboardEventHandler,
  type MouseEvent,
  type ReactNode,
  type TextareaHTMLAttributes,
} from 'react';
import { LegacyCloseCircleIcon } from './legacy-ant-icon';

type LegacyInputSize = 'small' | 'default' | 'large';
type LegacyInputValue = string | number | readonly string[] | null | undefined;

type LegacyInputProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'defaultValue' | 'onChange' | 'prefix' | 'size' | 'value'
> & {
  addonAfter?: ReactNode;
  addonBefore?: ReactNode;
  allowClear?: boolean;
  defaultValue?: LegacyInputValue;
  legacyAttributeOrder?: 'placeholder-first' | 'type-first';
  onChange?: ChangeEventHandler<HTMLInputElement>;
  onPressEnter?: KeyboardEventHandler<HTMLInputElement>;
  prefix?: ReactNode;
  prefixCls?: string;
  size?: LegacyInputSize;
  suffix?: ReactNode;
  value?: LegacyInputValue;
};

type LegacyTextAreaProps = Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  'defaultValue' | 'onChange' | 'prefix' | 'value'
> & {
  allowClear?: boolean;
  defaultValue?: LegacyInputValue;
  onChange?: ChangeEventHandler<HTMLTextAreaElement>;
  onPressEnter?: KeyboardEventHandler<HTMLTextAreaElement>;
  prefixCls?: string;
  value?: LegacyInputValue;
};

function classNames(...values: Array<string | false | null | undefined>) {
  const tokens: string[] = [];
  values.forEach((value) => {
    if (!value) return;
    value
      .split(/\s+/)
      .filter(Boolean)
      .forEach((token) => {
        if (!tokens.includes(token)) tokens.push(token);
      });
  });
  return tokens.join(' ') || undefined;
}

function normalizeValue(value: LegacyInputValue) {
  return value === undefined || value === null ? '' : value;
}

function hasValueProp(props: { value?: LegacyInputValue }) {
  return Object.prototype.hasOwnProperty.call(props, 'value');
}

function sizeClassName(prefixCls: string, size: LegacyInputSize | undefined) {
  if (size === 'small') return `${prefixCls}-sm`;
  if (size === 'large') return `${prefixCls}-lg`;
  return undefined;
}

function legacyInputClassName({
  className,
  disabled,
  includeCustomClassName = true,
  prefixCls,
  size,
}: {
  className: string | undefined;
  disabled: boolean | undefined;
  includeCustomClassName?: boolean;
  prefixCls: string;
  size?: LegacyInputSize;
}) {
  const inputClassName = classNames(
    prefixCls,
    sizeClassName(prefixCls, size),
    disabled && `${prefixCls}-disabled`,
    includeCustomClassName && className,
  );
  if (!inputClassName || !disabled) return inputClassName;
  const tokens = inputClassName.split(/\s+/).filter(Boolean);
  if (!tokens.includes(prefixCls) || tokens.includes(`${prefixCls}-disabled`)) {
    return inputClassName;
  }
  const insertAfter = Math.max(
    tokens.indexOf(prefixCls),
    tokens.indexOf(`${prefixCls}-sm`),
    tokens.indexOf(`${prefixCls}-lg`),
  );
  tokens.splice(insertAfter + 1, 0, `${prefixCls}-disabled`);
  return tokens.join(' ');
}

function useLegacyValue({
  controlled,
  defaultValue,
  value,
}: {
  controlled: boolean;
  defaultValue: LegacyInputValue;
  value: LegacyInputValue;
}) {
  const [innerValue, setInnerValue] = useState<LegacyInputValue>(() =>
    controlled ? value : defaultValue,
  );

  useEffect(() => {
    if (controlled) setInnerValue(value);
  }, [controlled, value]);

  return [normalizeValue(innerValue), setInnerValue, controlled] as const;
}

function callClearChange<T extends HTMLInputElement | HTMLTextAreaElement>(
  node: T,
  event: MouseEvent<HTMLElement>,
  onChange: ((event: ChangeEvent<T>) => void) | undefined,
  restoreValue: boolean,
) {
  if (!onChange) return;
  const previousValue = node.value;
  const nextEvent = Object.create(event) as ChangeEvent<T>;
  Object.defineProperty(nextEvent, 'target', { value: node });
  Object.defineProperty(nextEvent, 'currentTarget', { value: node });
  node.value = '';
  onChange(nextEvent);
  if (restoreValue) node.value = previousValue;
}

const LegacyInputBase = forwardRef<HTMLInputElement, LegacyInputProps>(function LegacyInput(
  props,
  ref,
) {
  const controlled = hasValueProp(props);
  const {
    addonAfter,
    addonBefore,
    allowClear,
    className,
    defaultValue = '',
    disabled,
    legacyAttributeOrder,
    onChange,
    onKeyDown,
    onPressEnter,
    placeholder,
    prefix,
    prefixCls = 'ant-input',
    readOnly,
    size,
    suffix,
    style,
    type = 'text',
    value,
    ...rest
  } = props;
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [mergedValue, setMergedValue] = useLegacyValue({ controlled, defaultValue, value });
  useImperativeHandle(ref, () => inputRef.current as HTMLInputElement, []);

  useLayoutEffect(() => {
    const node = inputRef.current;
    if (!node) return;
    const placeholderAttr = node.getAttribute('placeholder');
    const typeAttr = node.getAttribute('type') ?? type;
    const classAttr = node.getAttribute('class');
    const valueAttr = node.getAttribute('value') ?? '';
    const styleAttr = node.getAttribute('style');

    node.removeAttribute('placeholder');
    node.removeAttribute('type');
    node.removeAttribute('class');
    node.removeAttribute('value');
    node.removeAttribute('style');
    if (legacyAttributeOrder === 'type-first') {
      node.setAttribute('type', typeAttr);
      if (placeholderAttr !== null) node.setAttribute('placeholder', placeholderAttr);
    } else {
      if (placeholderAttr !== null) node.setAttribute('placeholder', placeholderAttr);
      node.setAttribute('type', typeAttr);
    }
    if (classAttr) node.setAttribute('class', classAttr);
    node.setAttribute('value', valueAttr);
    if (styleAttr !== null) node.setAttribute('style', styleAttr);
  });

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    if (!controlled) setMergedValue(event.target.value);
    onChange?.(event);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    if (event.keyCode === 13) onPressEnter?.(event);
    onKeyDown?.(event);
  };

  const handleReset = (event: MouseEvent<HTMLElement>) => {
    if (!inputRef.current) return;
    if (!controlled) setMergedValue('');
    inputRef.current.focus();
    callClearChange(inputRef.current, event, onChange, controlled);
  };

  const clearIcon =
    allowClear && !disabled && !readOnly && mergedValue !== '' ? (
      <LegacyCloseCircleIcon
        className={`${prefixCls}-clear-icon`}
        role="button"
        onClick={handleReset}
      />
    ) : null;
  const affixSuffix =
    suffix !== undefined || allowClear ? (
      <span className={`${prefixCls}-suffix`}>
        {clearIcon}
        {suffix}
      </span>
    ) : null;
  const hasAffix = prefix !== undefined || affixSuffix !== null;
  const hasAddon = addonBefore !== undefined || addonAfter !== undefined;

  const input = (
    <input
      ref={inputRef}
      placeholder={placeholder}
      type={type}
      className={legacyInputClassName({
        className,
        disabled,
        includeCustomClassName: !hasAddon,
        prefixCls,
        size,
      })}
      disabled={disabled}
      readOnly={readOnly}
      value={mergedValue}
      style={hasAffix || hasAddon ? undefined : style}
      onChange={handleChange}
      onKeyDown={handleKeyDown}
      {...rest}
    />
  );

  const affixInput = hasAffix ? (
    <span
      className={classNames(
        `${prefixCls}-affix-wrapper`,
        size === 'small' && `${prefixCls}-affix-wrapper-sm`,
        size === 'large' && `${prefixCls}-affix-wrapper-lg`,
        suffix !== undefined &&
          allowClear &&
          mergedValue !== '' &&
          `${prefixCls}-affix-wrapper-input-with-clear-btn`,
      )}
      style={hasAddon ? undefined : style}
    >
      {prefix !== undefined ? <span className={`${prefixCls}-prefix`}>{prefix}</span> : null}
      {input}
      {affixSuffix}
    </span>
  ) : (
    input
  );

  if (hasAddon) {
    return (
      <span
        className={classNames(
          className,
          `${prefixCls}-group-wrapper`,
          size === 'small' && `${prefixCls}-group-wrapper-sm`,
          size === 'large' && `${prefixCls}-group-wrapper-lg`,
        )}
        style={style}
      >
        <span className={`${prefixCls}-wrapper ${prefixCls}-group`}>
          {addonBefore !== undefined ? (
            <span className={`${prefixCls}-group-addon`}>{addonBefore}</span>
          ) : null}
          {affixInput}
          {addonAfter !== undefined ? (
            <span className={`${prefixCls}-group-addon`}>{addonAfter}</span>
          ) : null}
        </span>
      </span>
    );
  }

  return affixInput;
});

export const LegacyCheckboxInput = forwardRef<
  HTMLInputElement,
  Omit<InputHTMLAttributes<HTMLInputElement>, 'type'>
>(function LegacyCheckboxInput({ className, value = '', ...rest }, ref) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  useImperativeHandle(ref, () => inputRef.current as HTMLInputElement, []);

  useLayoutEffect(() => {
    const node = inputRef.current;
    if (!node) return;

    const valueAttr = node.getAttribute('value') ?? '';
    node.removeAttribute('type');
    node.removeAttribute('class');
    node.removeAttribute('value');
    node.setAttribute('type', 'checkbox');
    if (className) node.setAttribute('class', className);
    node.setAttribute('value', valueAttr);
  }, [className]);

  return (
    <input
      ref={inputRef}
      type="checkbox"
      className={className || undefined}
      value={value}
      {...rest}
    />
  );
});

export const LegacyTextArea = forwardRef<
  HTMLTextAreaElement,
  LegacyTextAreaProps
>(function LegacyTextArea(props, ref) {
  const controlled = hasValueProp(props);
  const {
    allowClear,
    className,
    defaultValue = '',
    disabled,
    onChange,
    onKeyDown,
    onPressEnter,
    placeholder,
    prefixCls = 'ant-input',
    readOnly,
    rows,
    style,
    value,
    ...rest
  } = props;
  const textAreaRef = useRef<HTMLTextAreaElement | null>(null);
  const [mergedValue, setMergedValue] = useLegacyValue({ controlled, defaultValue, value });
  useImperativeHandle(ref, () => textAreaRef.current as HTMLTextAreaElement, []);

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    if (!controlled) setMergedValue(event.target.value);
    onChange?.(event);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    if (event.keyCode === 13) onPressEnter?.(event);
    onKeyDown?.(event);
  };

  const handleReset = (event: MouseEvent<HTMLElement>) => {
    if (!textAreaRef.current) return;
    if (!controlled) setMergedValue('');
    textAreaRef.current.focus();
    callClearChange(textAreaRef.current, event, onChange, controlled);
  };

  const textarea = (
    <textarea
      ref={textAreaRef}
      rows={rows}
      placeholder={placeholder}
      className={legacyInputClassName({ className, disabled, prefixCls })}
      disabled={disabled}
      readOnly={readOnly}
      value={mergedValue}
      style={allowClear ? undefined : style}
      onChange={handleChange}
      onKeyDown={handleKeyDown}
      {...rest}
    />
  );

  if (!allowClear) return textarea;

  return (
    <span
      className={classNames(
        className,
        `${prefixCls}-affix-wrapper`,
        `${prefixCls}-affix-wrapper-textarea-with-clear-btn`,
      )}
      style={style}
    >
      {textarea}
      {!disabled && !readOnly && mergedValue !== '' ? (
        <LegacyCloseCircleIcon
          className={`${prefixCls}-textarea-clear-icon`}
          role="button"
          onClick={handleReset}
        />
      ) : null}
    </span>
  );
});

export function LegacyInputCompactGroup({
  children,
  className,
  prefixCls = 'ant-input-group',
  size,
  style,
}: {
  children: ReactNode;
  className?: string;
  prefixCls?: string;
  size?: LegacyInputSize;
  style?: CSSProperties;
}) {
  return (
    <LegacyInputStaticGroup
      className={className}
      compact
      prefixCls={prefixCls}
      size={size}
      style={style}
    >
      {children}
    </LegacyInputStaticGroup>
  );
}

function LegacyInputStaticGroup({
  children,
  className,
  compact = false,
  prefixCls = 'ant-input-group',
  size,
  style,
}: {
  children: ReactNode;
  className?: string;
  compact?: boolean;
  prefixCls?: string;
  size?: LegacyInputSize;
  style?: CSSProperties;
}) {
  return (
    <span
      className={classNames(
        prefixCls,
        size === 'small' && `${prefixCls}-sm`,
        size === 'large' && `${prefixCls}-lg`,
        compact && `${prefixCls}-compact`,
        className,
      )}
      style={style}
    >
      {children}
    </span>
  );
}

export const LegacyInput = Object.assign(LegacyInputBase, {
  TextArea: LegacyTextArea,
  Group: LegacyInputStaticGroup,
});

export function LegacyInputGroup({
  addonBefore,
  addonAfter,
  className,
  defaultValue,
  disabled,
  onChange,
  placeholder,
  size,
  type = 'text',
  value,
}: {
  addonBefore?: ReactNode;
  addonAfter?: ReactNode;
  className?: string;
  defaultValue?: string | number | readonly string[] | undefined;
  disabled?: boolean;
  onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
  placeholder?: string;
  size?: LegacyInputSize;
  type?: HTMLInputElement['type'];
  value?: string | number | readonly string[] | undefined;
}) {
  const wrapperClassName = classNames(
    'ant-input-group-wrapper',
    size === 'small' && 'ant-input-group-wrapper-sm',
    size === 'large' && 'ant-input-group-wrapper-lg',
  );
  const inputClassName =
    className ??
    classNames(
      'ant-input',
      size === 'small' && 'ant-input-sm',
      size === 'large' && 'ant-input-lg',
    );

  return (
    <span className={wrapperClassName}>
      <span className="ant-input-wrapper ant-input-group">
        {addonBefore !== undefined ? (
          <span className="ant-input-group-addon">{addonBefore}</span>
        ) : null}
        <LegacyInput
          placeholder={placeholder}
          type={type}
          className={inputClassName}
          disabled={disabled}
          defaultValue={defaultValue}
          legacyAttributeOrder={type === 'number' ? 'type-first' : undefined}
          onChange={onChange}
          {...(value !== undefined ? { value } : {})}
        />
        {addonAfter !== undefined ? (
          <span className="ant-input-group-addon">{addonAfter}</span>
        ) : null}
      </span>
    </span>
  );
}
