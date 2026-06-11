import {
  useEffect,
  useState,
  isValidElement,
  type HTMLAttributes,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
} from 'react';
import { LegacyCloseIcon } from './legacy-ant-icon';
import { triggerLegacyWave } from './legacy-wave';

const LEGACY_TAG_PRESET_COLORS = [
  'pink',
  'red',
  'yellow',
  'orange',
  'cyan',
  'green',
  'blue',
  'purple',
  'geekblue',
  'magenta',
  'volcano',
  'gold',
  'lime',
];
const LEGACY_TAG_PRESET_COLOR_RE = new RegExp(
  `^(${LEGACY_TAG_PRESET_COLORS.join('|')})(-inverse)?$`,
);

interface LegacyTagProps extends Omit<HTMLAttributes<HTMLSpanElement>, 'color' | 'onClose'> {
  afterClose?: () => void;
  children?: ReactNode;
  closable?: boolean;
  color?: string;
  onClose?: (event: MouseEvent<HTMLElement>) => void;
  prefixCls?: string;
  visible?: boolean;
}

interface LegacyCheckableTagProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, 'onChange'> {
  checked?: boolean;
  children?: ReactNode;
  onChange?: (checked: boolean) => void;
  prefixCls?: string;
}

function isPresetColor(color?: string) {
  return !!color && LEGACY_TAG_PRESET_COLOR_RE.test(color);
}

function isSingleAnchorChild(children: ReactNode) {
  return isValidElement(children) && (children as ReactElement).type === 'a';
}

function mergeClassName(...tokens: Array<string | false | null | undefined>) {
  return tokens.filter(Boolean).join(' ');
}

function LegacyCheckableTag({
  checked,
  children,
  className,
  onChange,
  prefixCls = 'ant-tag',
  ...restProps
}: LegacyCheckableTagProps) {
  const tagClassName = mergeClassName(
    prefixCls,
    `${prefixCls}-checkable`,
    checked ? `${prefixCls}-checkable-checked` : null,
    className,
  );

  const handleClick = () => {
    onChange?.(!checked);
  };

  return (
    <span {...restProps} className={tagClassName} onClick={handleClick}>
      {children}
    </span>
  );
}

function LegacyTagBase(props: LegacyTagProps) {
  const hasControlledVisible = Object.prototype.hasOwnProperty.call(props, 'visible');
  const {
    afterClose,
    children,
    className,
    closable = false,
    color,
    onClose,
    prefixCls = 'ant-tag',
    style,
    visible,
    ...restProps
  } = props;
  const [internalVisible, setInternalVisible] = useState(hasControlledVisible ? visible : true);

  useEffect(() => {
    if (hasControlledVisible) {
      setInternalVisible(visible);
    }
  }, [hasControlledVisible, visible]);

  const tagVisible = hasControlledVisible ? visible : internalVisible;
  const presetColor = isPresetColor(color);
  const tagClassName = mergeClassName(
    prefixCls,
    presetColor ? `${prefixCls}-${color}` : null,
    color && !presetColor ? `${prefixCls}-has-color` : null,
    !tagVisible ? `${prefixCls}-hidden` : null,
    className,
  );
  const tagStyle = { backgroundColor: color && !presetColor ? color : undefined, ...style };
  const shouldTriggerWave =
    Object.prototype.hasOwnProperty.call(restProps, 'onClick') || isSingleAnchorChild(children);

  const close = (event: MouseEvent<HTMLElement>) => {
    event.stopPropagation();
    onClose?.(event);
    if (!onClose) afterClose?.();
    if (!event.defaultPrevented && !hasControlledVisible) {
      setInternalVisible(false);
    }
  };

  const handleClick = shouldTriggerWave
    ? (event: MouseEvent<HTMLSpanElement>) => {
        triggerLegacyWave(event.currentTarget);
        restProps.onClick?.(event);
      }
    : restProps.onClick;

  return (
    <span {...restProps} className={tagClassName} style={tagStyle} onClick={handleClick}>
      {children}
      {closable ? <LegacyCloseIcon onClick={close} /> : null}
    </span>
  );
}

export const LegacyTag = Object.assign(LegacyTagBase, {
  CheckableTag: LegacyCheckableTag,
});
