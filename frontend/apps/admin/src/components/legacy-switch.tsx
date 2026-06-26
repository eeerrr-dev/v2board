import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import { LegacyLoadingIcon } from './legacy-ant-icon';

type LegacySwitchEvent =
  | ReactKeyboardEvent<HTMLButtonElement>
  | ReactMouseEvent<HTMLButtonElement>;

export interface LegacySwitchRef {
  blur: () => void;
  focus: () => void;
}

interface LegacySwitchProps
  extends Omit<
    ButtonHTMLAttributes<HTMLButtonElement>,
    | 'checked'
    | 'children'
    | 'defaultChecked'
    | 'disabled'
    | 'onChange'
    | 'onClick'
    | 'onMouseUp'
    | 'size'
    | 'type'
  > {
  checked?: boolean | number | string;
  checkedChildren?: ReactNode;
  defaultChecked?: boolean | number | string;
  disabled?: boolean;
  loading?: boolean;
  onClick?: (checked: boolean, event: ReactMouseEvent<HTMLButtonElement>) => void;
  onChange?: (checked: boolean, event: LegacySwitchEvent) => void;
  onMouseUp?: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  prefixCls?: string;
  size?: 'small' | 'default' | 'large';
  unCheckedChildren?: ReactNode;
}

function mergeClassName(...values: Array<string | undefined | false>) {
  return values.filter(Boolean).join(' ');
}

export const LegacySwitch = forwardRef<LegacySwitchRef, LegacySwitchProps>(function LegacySwitch(
  props,
  ref,
) {
  const {
    autoFocus,
    checked,
    checkedChildren,
    className,
    defaultChecked = false,
    disabled = false,
    loading = false,
    onClick,
    onChange,
    onMouseUp,
    prefixCls = 'ant-switch',
    size,
    unCheckedChildren,
    ...restProps
  } = props;
  const isControlled = Object.prototype.hasOwnProperty.call(props, 'checked');
  const [innerChecked, setInnerChecked] = useState(
    () => (isControlled ? !!checked : !!defaultChecked),
  );
  const enabled = isControlled ? !!checked : innerChecked;
  const switchDisabled = disabled || loading;
  const nodeRef = useRef<HTMLButtonElement | null>(null);

  useImperativeHandle(ref, () => ({
    blur: () => nodeRef.current?.blur(),
    focus: () => nodeRef.current?.focus(),
  }));

  useEffect(() => {
    if (autoFocus && !switchDisabled) nodeRef.current?.focus();
  }, []);

  const setChecked = (nextChecked: boolean, event: LegacySwitchEvent) => {
    if (switchDisabled) return;
    if (!isControlled) setInnerChecked(nextChecked);
    onChange?.(nextChecked, event);
  };

  const handleClick = (event: ReactMouseEvent<HTMLButtonElement>) => {
    const nextChecked = !enabled;
    setChecked(nextChecked, event);
    onClick?.(nextChecked, event);
  };

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>) => {
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    if (event.keyCode === 37) {
      setChecked(false, event);
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
    } else if (event.keyCode === 39) {
      setChecked(true, event);
    }
  };

  const handleMouseUp = (event: ReactMouseEvent<HTMLButtonElement>) => {
    nodeRef.current?.blur();
    onMouseUp?.(event);
  };

  return (
    <button
      {...restProps}
      ref={nodeRef}
      type="button"
      role="switch"
      aria-checked={enabled}
      autoFocus={autoFocus}
      disabled={switchDisabled}
      className={mergeClassName(
        className,
        size === 'small' && `${prefixCls}-small`,
        loading && `${prefixCls}-loading`,
        prefixCls,
        enabled && `${prefixCls}-checked`,
        switchDisabled && `${prefixCls}-disabled`,
      )}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      onMouseUp={handleMouseUp}
    >
      {loading ? <LegacyLoadingIcon className={`${prefixCls}-loading-icon`} /> : null}
      <span className={`${prefixCls}-inner`}>
        {enabled ? checkedChildren : unCheckedChildren}
      </span>
    </button>
  );
});

LegacySwitch.displayName = 'LegacySwitch';
(LegacySwitch as typeof LegacySwitch & { __ANT_SWITCH: boolean }).__ANT_SWITCH = true;
