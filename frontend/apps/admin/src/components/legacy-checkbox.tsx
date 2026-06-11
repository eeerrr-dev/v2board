import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ChangeEvent,
  type CSSProperties,
  type HTMLAttributes,
  type InputHTMLAttributes,
  type MouseEventHandler,
  type ReactNode,
} from 'react';

type LegacyCheckboxValue = string | number | boolean;

export type LegacyCheckboxChangeEvent = ChangeEvent<HTMLInputElement> & {
  target: HTMLInputElement & {
    checked: boolean;
    value: LegacyCheckboxValue | undefined;
  };
};

type LegacyCheckboxGroupContextValue = {
  cancelValue: (value: LegacyCheckboxValue | undefined) => void;
  disabled?: boolean;
  name?: string;
  registerValue: (value: LegacyCheckboxValue | undefined) => void;
  toggleOption: (option: { label: ReactNode; value: LegacyCheckboxValue | undefined }) => void;
  value: LegacyCheckboxValue[];
};

export type LegacyCheckboxProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  | 'children'
  | 'onChange'
  | 'onMouseEnter'
  | 'onMouseLeave'
  | 'prefix'
  | 'size'
  | 'style'
  | 'type'
  | 'value'
> & {
  children?: ReactNode;
  className?: string;
  indeterminate?: boolean;
  onChange?: (event: LegacyCheckboxChangeEvent) => void;
  onMouseEnter?: MouseEventHandler<HTMLLabelElement>;
  onMouseLeave?: MouseEventHandler<HTMLLabelElement>;
  prefixCls?: string;
  style?: CSSProperties;
  value?: LegacyCheckboxValue;
};

export type LegacyCheckboxOption =
  | string
  | {
      disabled?: boolean;
      label: ReactNode;
      onChange?: (event: LegacyCheckboxChangeEvent) => void;
      value: LegacyCheckboxValue;
    };

export type LegacyCheckboxGroupProps = Omit<
  HTMLAttributes<HTMLDivElement>,
  'children' | 'defaultValue' | 'onChange' | 'style'
> & {
  children?: ReactNode;
  defaultValue?: LegacyCheckboxValue[];
  disabled?: boolean;
  name?: string;
  onChange?: (checkedValue: LegacyCheckboxValue[]) => void;
  options?: LegacyCheckboxOption[];
  prefixCls?: string;
  style?: CSSProperties;
  value?: LegacyCheckboxValue[];
};

const LegacyCheckboxGroupContext = createContext<LegacyCheckboxGroupContextValue | null>(null);

function joinClassNames(...tokens: Array<string | false | null | undefined>) {
  return tokens.filter(Boolean).join(' ');
}

function makeLegacyCheckboxChangeEvent(
  event: ChangeEvent<HTMLInputElement>,
  value: LegacyCheckboxValue | undefined,
): LegacyCheckboxChangeEvent {
  const target = {
    checked: event.target.checked,
    disabled: event.target.disabled,
    name: event.target.name,
    type: event.target.type,
    value,
  };

  return Object.assign(Object.create(Object.getPrototypeOf(event)), event, {
    currentTarget: target,
    target,
  }) as LegacyCheckboxChangeEvent;
}

function optionValue(option: LegacyCheckboxOption) {
  return typeof option === 'string' ? option : option.value;
}

function optionLabel(option: LegacyCheckboxOption) {
  return typeof option === 'string' ? option : option.label;
}

function optionDisabled(option: LegacyCheckboxOption) {
  return typeof option === 'string' ? undefined : option.disabled;
}

const LegacyCheckboxBase = forwardRef<HTMLInputElement, LegacyCheckboxProps>(
  function LegacyCheckbox(
    {
      checked,
      children,
      className,
      defaultChecked,
      disabled,
      indeterminate = false,
      name,
      onChange,
      onMouseEnter,
      onMouseLeave,
      prefixCls = 'ant-checkbox',
      style,
      value,
      ...rest
    },
    ref,
  ) {
    const checkboxGroup = useContext(LegacyCheckboxGroupContext);
    const [innerChecked, setInnerChecked] = useState(Boolean(defaultChecked));
    const mergedDisabled = disabled || checkboxGroup?.disabled;
    const mergedChecked = checkboxGroup
      ? checkboxGroup.value.includes(value as LegacyCheckboxValue)
      : checked !== undefined
        ? checked
        : innerChecked;
    const wrapperClassName = joinClassNames(
      className,
      `${prefixCls}-wrapper`,
      mergedChecked && `${prefixCls}-wrapper-checked`,
      mergedDisabled && `${prefixCls}-wrapper-disabled`,
    );
    const checkboxClassName = joinClassNames(
      prefixCls,
      mergedChecked && `${prefixCls}-checked`,
      mergedDisabled && `${prefixCls}-disabled`,
      indeterminate && `${prefixCls}-indeterminate`,
    );

    useEffect(() => {
      if (checked !== undefined) setInnerChecked(checked);
    }, [checked]);

    useEffect(() => {
      if (!checkboxGroup) return undefined;
      checkboxGroup.registerValue(value);
      return () => checkboxGroup.cancelValue(value);
    }, [checkboxGroup?.cancelValue, checkboxGroup?.registerValue, value]);

    const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
      if (!checkboxGroup && checked === undefined) setInnerChecked(event.target.checked);

      const legacyEvent = makeLegacyCheckboxChangeEvent(event, value);
      onChange?.(legacyEvent);
      checkboxGroup?.toggleOption({ label: children, value });
    };

    return (
      <label
        className={wrapperClassName}
        style={style}
        onMouseEnter={onMouseEnter}
        onMouseLeave={onMouseLeave}
      >
        <span className={checkboxClassName}>
          <input
            {...rest}
            ref={ref}
            name={checkboxGroup?.name ?? name}
            type="checkbox"
            className={`${prefixCls}-input`}
            checked={mergedChecked}
            disabled={mergedDisabled}
            value={value === undefined ? '' : String(value)}
            onChange={handleChange}
          />
          <span className={`${prefixCls}-inner`} />
        </span>
        {children !== undefined ? <span>{children}</span> : null}
      </label>
    );
  },
);

function LegacyCheckboxGroup({
  children,
  className,
  defaultValue = [],
  disabled,
  name,
  onChange,
  options = [],
  prefixCls = 'ant-checkbox',
  style,
  value,
  ...rest
}: LegacyCheckboxGroupProps) {
  const [innerValue, setInnerValue] = useState<LegacyCheckboxValue[]>(value ?? defaultValue);
  const [registeredValues, setRegisteredValues] = useState<Array<LegacyCheckboxValue | undefined>>(
    [],
  );
  const mergedValue = value ?? innerValue;
  const groupPrefixCls = `${prefixCls}-group`;

  useEffect(() => {
    if (value) setInnerValue(value);
  }, [value]);

  const availableValues = options.length > 0 ? options.map(optionValue) : registeredValues;

  const registerValue = useCallback((nextValue: LegacyCheckboxValue | undefined) => {
    setRegisteredValues((current) =>
      current.includes(nextValue) ? current : [...current, nextValue],
    );
  }, []);

  const cancelValue = useCallback((nextValue: LegacyCheckboxValue | undefined) => {
    setRegisteredValues((current) => current.filter((item) => item !== nextValue));
  }, []);

  const toggleOption = useCallback(
    (option: { label: ReactNode; value: LegacyCheckboxValue | undefined }) => {
      const currentIndex = mergedValue.indexOf(option.value as LegacyCheckboxValue);
      const nextValue = [...mergedValue];
      if (currentIndex === -1) {
        nextValue.push(option.value as LegacyCheckboxValue);
      } else {
        nextValue.splice(currentIndex, 1);
      }

      const orderedValue = nextValue
        .filter((item) => availableValues.includes(item))
        .sort((left, right) => availableValues.indexOf(left) - availableValues.indexOf(right));

      if (value === undefined) setInnerValue(orderedValue);
      onChange?.(orderedValue);
    },
    [availableValues, mergedValue, onChange, value],
  );

  const contextValue = useMemo<LegacyCheckboxGroupContextValue>(
    () => ({
      cancelValue,
      disabled,
      name,
      registerValue,
      toggleOption,
      value: mergedValue,
    }),
    [cancelValue, disabled, name, registerValue, toggleOption, mergedValue],
  );
  const mergedChildren =
    options.length > 0
      ? options.map((option) => (
          <LegacyCheckboxBase
            key={String(optionValue(option))}
            checked={mergedValue.includes(optionValue(option))}
            className={`${groupPrefixCls}-item`}
            disabled={optionDisabled(option) ?? disabled}
            onChange={typeof option === 'string' ? undefined : option.onChange}
            prefixCls={prefixCls}
            value={optionValue(option)}
          >
            {optionLabel(option)}
          </LegacyCheckboxBase>
        ))
      : children;

  return (
    <LegacyCheckboxGroupContext.Provider value={contextValue}>
      <div {...rest} className={joinClassNames(groupPrefixCls, className)} style={style}>
        {mergedChildren}
      </div>
    </LegacyCheckboxGroupContext.Provider>
  );
}

export const LegacyCheckbox = Object.assign(LegacyCheckboxBase, {
  Group: LegacyCheckboxGroup,
});

(LegacyCheckbox as typeof LegacyCheckbox & { __ANT_CHECKBOX: boolean }).__ANT_CHECKBOX = true;
