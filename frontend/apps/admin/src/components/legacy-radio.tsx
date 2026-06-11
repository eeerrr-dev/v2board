import {
  Children,
  createContext,
  forwardRef,
  isValidElement,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ChangeEvent,
  type CSSProperties,
  type InputHTMLAttributes,
  type MouseEventHandler,
  type ReactNode,
} from 'react';

type LegacyRadioValue = string | number | boolean;
type LegacyRadioButtonStyle = 'outline' | 'solid';
type LegacyRadioSize = 'large' | 'default' | 'small';

export type LegacyRadioChangeEvent = ChangeEvent<HTMLInputElement> & {
  target: HTMLInputElement & {
    checked: boolean;
    value: LegacyRadioValue;
  };
};

type LegacyRadioGroupContextValue = {
  disabled?: boolean;
  name?: string;
  onChange: (event: ChangeEvent<HTMLInputElement>, value: LegacyRadioValue | undefined) => void;
  value?: LegacyRadioValue;
};

export type LegacyRadioProps = Omit<
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
  onChange?: (event: LegacyRadioChangeEvent) => void;
  onMouseEnter?: MouseEventHandler<HTMLLabelElement>;
  onMouseLeave?: MouseEventHandler<HTMLLabelElement>;
  prefixCls?: string;
  style?: CSSProperties;
  value?: LegacyRadioValue;
};

export type LegacyRadioOption =
  | string
  | {
      disabled?: boolean;
      label: ReactNode;
      value: LegacyRadioValue;
    };

export type LegacyRadioGroupProps = {
  buttonStyle?: LegacyRadioButtonStyle;
  children?: ReactNode;
  className?: string;
  defaultValue?: LegacyRadioValue;
  disabled?: boolean;
  id?: string;
  name?: string;
  onChange?: (event: LegacyRadioChangeEvent) => void;
  onMouseEnter?: MouseEventHandler<HTMLDivElement>;
  onMouseLeave?: MouseEventHandler<HTMLDivElement>;
  options?: LegacyRadioOption[];
  prefixCls?: string;
  size?: LegacyRadioSize;
  style?: CSSProperties;
  value?: LegacyRadioValue;
};

const LegacyRadioGroupContext = createContext<LegacyRadioGroupContextValue | null>(null);

function joinClassNames(...tokens: Array<string | false | null | undefined>) {
  return tokens.filter(Boolean).join(' ');
}

function getCheckedValueFromChildren(children: ReactNode) {
  let value: LegacyRadioValue | undefined;
  let found = false;

  Children.forEach(children, (child) => {
    if (!found && isValidElement<LegacyRadioProps>(child) && child.props.checked) {
      value = child.props.value;
      found = true;
    }
  });

  return found ? value : undefined;
}

function makeLegacyRadioChangeEvent(
  event: ChangeEvent<HTMLInputElement>,
  value: LegacyRadioValue | undefined,
): LegacyRadioChangeEvent {
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
  }) as LegacyRadioChangeEvent;
}

const LegacyRadioBase = forwardRef<HTMLInputElement, LegacyRadioProps>(function LegacyRadio(
  {
    checked,
    children,
    className,
    defaultChecked,
    disabled,
    name,
    onChange,
    onMouseEnter,
    onMouseLeave,
    prefixCls = 'ant-radio',
    style,
    value,
    ...rest
  },
  ref,
) {
  const radioGroup = useContext(LegacyRadioGroupContext);
  const [innerChecked, setInnerChecked] = useState(Boolean(defaultChecked));
  const mergedDisabled = disabled || radioGroup?.disabled;
  const mergedChecked = radioGroup
    ? value === radioGroup.value
    : checked !== undefined
      ? checked
      : innerChecked;
  const wrapperClassName = joinClassNames(
    className,
    `${prefixCls}-wrapper`,
    mergedChecked && `${prefixCls}-wrapper-checked`,
    mergedDisabled && `${prefixCls}-wrapper-disabled`,
  );
  const radioClassName = joinClassNames(
    prefixCls,
    mergedChecked && `${prefixCls}-checked`,
    mergedDisabled && `${prefixCls}-disabled`,
  );

  useEffect(() => {
    if (checked !== undefined) setInnerChecked(checked);
  }, [checked]);

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    if (!radioGroup && checked === undefined) setInnerChecked(event.target.checked);

    const legacyEvent = makeLegacyRadioChangeEvent(event, value);
    onChange?.(legacyEvent);
    radioGroup?.onChange(event, value);
  };

  return (
    <label
      className={wrapperClassName}
      style={style}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
    >
      <span className={radioClassName}>
        <input
          {...rest}
          ref={ref}
          name={radioGroup?.name ?? name}
          type="radio"
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
});

function LegacyRadioButton(props: LegacyRadioProps) {
  const { prefixCls = 'ant-radio-button', ...rest } = props;
  return <LegacyRadioBase {...rest} prefixCls={prefixCls} />;
}

function LegacyRadioGroup({
  buttonStyle = 'outline',
  children,
  className,
  defaultValue,
  disabled,
  id,
  name,
  onChange,
  onMouseEnter,
  onMouseLeave,
  options,
  prefixCls = 'ant-radio',
  size,
  style,
  value,
}: LegacyRadioGroupProps) {
  const initialValue =
    value !== undefined
      ? value
      : defaultValue !== undefined
        ? defaultValue
        : getCheckedValueFromChildren(children);
  const [innerValue, setInnerValue] = useState<LegacyRadioValue | undefined>(initialValue);
  const mergedValue = value !== undefined ? value : innerValue;
  const groupPrefixCls = `${prefixCls}-group`;
  const groupClassName = joinClassNames(
    groupPrefixCls,
    `${groupPrefixCls}-${buttonStyle}`,
    size && `${groupPrefixCls}-${size}`,
    className,
  );

  useEffect(() => {
    if (value !== undefined) {
      setInnerValue(value);
      return;
    }

    const checkedValue = getCheckedValueFromChildren(children);
    if (checkedValue !== undefined) setInnerValue(checkedValue);
  }, [children, value]);

  const handleChange = (event: ChangeEvent<HTMLInputElement>, nextValue: LegacyRadioValue | undefined) => {
    if (nextValue === mergedValue) return;
    if (value === undefined) setInnerValue(nextValue);
    onChange?.(makeLegacyRadioChangeEvent(event, nextValue));
  };

  const contextValue = useMemo<LegacyRadioGroupContextValue>(
    () => ({
      disabled,
      name,
      onChange: handleChange,
      value: mergedValue,
    }),
    [disabled, name, mergedValue],
  );
  const mergedChildren =
    options && options.length > 0
      ? options.map((option) =>
          typeof option === 'string' ? (
            <LegacyRadioBase
              key={option}
              disabled={disabled}
              prefixCls={prefixCls}
              value={option}
            >
              {option}
            </LegacyRadioBase>
          ) : (
            <LegacyRadioBase
              key={`radio-group-value-options-${String(option.value)}`}
              disabled={option.disabled || disabled}
              prefixCls={prefixCls}
              value={option.value}
            >
              {option.label}
            </LegacyRadioBase>
          ),
        )
      : children;

  return (
    <LegacyRadioGroupContext.Provider value={contextValue}>
      <div
        className={groupClassName}
        style={style}
        onMouseEnter={onMouseEnter}
        onMouseLeave={onMouseLeave}
        id={id}
      >
        {mergedChildren}
      </div>
    </LegacyRadioGroupContext.Provider>
  );
}

export const LegacyRadio = Object.assign(LegacyRadioBase, {
  Button: LegacyRadioButton,
  Group: LegacyRadioGroup,
});
