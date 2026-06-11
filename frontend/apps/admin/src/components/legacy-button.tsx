import {
  Children,
  cloneElement,
  forwardRef,
  isValidElement,
  useEffect,
  useRef,
  useState,
  type AnchorHTMLAttributes,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type HTMLAttributes,
  type MouseEvent,
  type Ref,
  type ReactElement,
  type ReactNode,
} from 'react';
import { LegacyAntIcon, LegacyLoadingIcon, type LegacyAntIconName } from './legacy-ant-icon';
import { triggerLegacyWave } from './legacy-wave';

const TWO_CN_CHAR = /^[一-龥]{2}$/;
const NATIVE_BUTTON_TYPES = new Set(['button', 'submit', 'reset']);

type LegacyButtonStyleType = 'default' | 'primary' | 'ghost' | 'dashed' | 'danger' | 'link';
type LegacyButtonShape = 'circle' | 'circle-outline' | 'round';
type LegacyButtonSize = 'large' | 'default' | 'small';
type LegacyNativeButtonType = NonNullable<ButtonHTMLAttributes<HTMLButtonElement>['type']>;
type LegacyButtonLoading = boolean | { delay?: number };

type LegacyButtonAnchorProps = Pick<
  AnchorHTMLAttributes<HTMLAnchorElement>,
  'download' | 'href' | 'hrefLang' | 'media' | 'ping' | 'referrerPolicy' | 'rel' | 'target'
>;

export type LegacyButtonProps = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'children' | 'onClick' | 'style' | 'type'
> &
  LegacyButtonAnchorProps & {
    autoInsertSpaceInButton?: boolean;
    block?: boolean;
    children?: ReactNode;
    className?: string;
    ghost?: boolean;
    htmlType?: LegacyNativeButtonType;
    icon?: LegacyAntIconName;
    loading?: LegacyButtonLoading;
    onClick?: (event: MouseEvent<HTMLElement>) => void;
    prefixCls?: string;
    shape?: LegacyButtonShape;
    size?: LegacyButtonSize;
    style?: CSSProperties;
    type?: LegacyButtonStyleType | LegacyNativeButtonType;
  };

export type LegacyButtonGroupProps = Omit<HTMLAttributes<HTMLDivElement>, 'className'> & {
  className?: string;
  prefixCls?: string;
  size?: LegacyButtonSize;
};

function isStringOrNumber(value: ReactNode): value is string | number {
  return typeof value === 'string' || typeof value === 'number';
}

function isStringElement(
  value: ReactNode,
): value is ReactElement<{ children?: ReactNode }, string> {
  return isValidElement(value) && typeof value.type === 'string';
}

function maybeInsertSpace(child: ReactNode, needSpace: boolean): ReactNode {
  const space = needSpace ? ' ' : '';
  if (
    isStringElement(child) &&
    typeof child.props.children === 'string' &&
    TWO_CN_CHAR.test(child.props.children)
  ) {
    return cloneElement(child, {}, child.props.children.split('').join(space));
  }
  if (typeof child === 'string') {
    return <span>{TWO_CN_CHAR.test(child) ? child.split('').join(space) : child}</span>;
  }
  return child;
}

function insertSpace(children: ReactNode, needSpace: boolean): ReactNode {
  let prevIsText = false;
  const merged: ReactNode[] = [];
  Children.forEach(children, (child) => {
    if (prevIsText && isStringOrNumber(child)) {
      const lastIndex = merged.length - 1;
      merged[lastIndex] = `${merged[lastIndex] ?? ''}${child}`;
    } else {
      merged.push(child);
    }
    prevIsText = isStringOrNumber(child);
  });
  return Children.map(merged, (child) => maybeInsertSpace(child, needSpace));
}

function shouldInsertSpace(children: ReactNode) {
  let count = 0;
  Children.forEach(children, (child) => {
    if (child !== null && child !== undefined && typeof child !== 'boolean') count += 1;
  });
  return count === 1;
}

function hasLegacyChildren(children: ReactNode) {
  return Boolean(children) || children === 0;
}

function hasClassToken(className: string | undefined, token: string) {
  return className?.split(/\s+/).includes(token) ?? false;
}

function addClassToken(tokens: string[], token: string | undefined) {
  if (token && !tokens.includes(token)) tokens.push(token);
}

function getSizeClassName(prefixCls: string, size: LegacyButtonSize | undefined) {
  if (size === 'large') return `${prefixCls}-lg`;
  if (size === 'small') return `${prefixCls}-sm`;
  return undefined;
}

function isNativeButtonType(type: LegacyButtonProps['type']): type is LegacyNativeButtonType {
  return type !== undefined && NATIVE_BUTTON_TYPES.has(type);
}

function isDelayedLoading(loading: LegacyButtonLoading | undefined): loading is { delay?: number } {
  return (
    loading !== undefined &&
    loading !== false &&
    typeof loading !== 'boolean' &&
    Boolean(loading.delay)
  );
}

function mergeRefs<T>(externalRef: Ref<T> | undefined, internalRef: { current: T | null }) {
  return (node: T | null) => {
    internalRef.current = node;
    if (typeof externalRef === 'function') {
      externalRef(node);
    } else if (externalRef) {
      externalRef.current = node;
    }
  };
}

function getButtonClassName({
  autoInsertSpaceInButton,
  block,
  buttonType,
  className,
  ghost,
  hasTwoCNChar,
  iconOnly,
  loading,
  prefixCls,
  shape,
  size,
}: {
  autoInsertSpaceInButton: boolean;
  block: boolean | undefined;
  buttonType: LegacyButtonStyleType | undefined;
  className: string | undefined;
  ghost: boolean | undefined;
  hasTwoCNChar: boolean;
  iconOnly: boolean;
  loading: boolean;
  prefixCls: string;
  shape: LegacyButtonShape | undefined;
  size: LegacyButtonSize | undefined;
}) {
  const tokens: string[] = [];
  addClassToken(tokens, prefixCls);
  className?.split(/\s+/).filter(Boolean).forEach((token) => addClassToken(tokens, token));
  addClassToken(tokens, buttonType ? `${prefixCls}-${buttonType}` : undefined);
  addClassToken(tokens, shape ? `${prefixCls}-${shape}` : undefined);
  addClassToken(tokens, getSizeClassName(prefixCls, size));
  addClassToken(tokens, iconOnly ? `${prefixCls}-icon-only` : undefined);
  addClassToken(tokens, loading ? `${prefixCls}-loading` : undefined);
  addClassToken(tokens, ghost ? `${prefixCls}-background-ghost` : undefined);
  addClassToken(
    tokens,
    hasTwoCNChar && autoInsertSpaceInButton ? `${prefixCls}-two-chinese-chars` : undefined,
  );
  addClassToken(tokens, block ? `${prefixCls}-block` : undefined);
  return tokens.join(' ');
}

const LegacyButtonBase = forwardRef<HTMLButtonElement | HTMLAnchorElement, LegacyButtonProps>(
  function LegacyButton(
    {
      autoInsertSpaceInButton = true,
      block,
      children,
      className,
      ghost,
      htmlType,
      icon,
      loading,
      onClick,
      prefixCls = 'ant-btn',
      shape,
      size,
      style,
      type,
      ...rest
    },
    ref,
  ) {
    const nodeRef = useRef<HTMLButtonElement | HTMLAnchorElement | null>(null);
    const [loadingState, setLoadingState] = useState<LegacyButtonLoading>(() => loading ?? false);
    const [hasTwoCNChar, setHasTwoCNChar] = useState(false);
    const visualLoading = Boolean(loadingState);
    const buttonType = isNativeButtonType(type) ? undefined : type;
    const classNameLoading =
      hasClassToken(className, `${prefixCls}-loading`) ||
      hasClassToken(className, 'ant-btn-loading');
    const clickLoading = visualLoading || classNameLoading;
    const needInserted =
      shouldInsertSpace(children) && !icon && buttonType !== 'link' && autoInsertSpaceInButton;
    const buttonIcon = visualLoading ? (
      <LegacyLoadingIcon />
    ) : icon ? (
      <LegacyAntIcon name={icon} />
    ) : null;
    const content = hasLegacyChildren(children) ? insertSpace(children, needInserted) : null;
    const iconOnly = !hasLegacyChildren(children) && (visualLoading || Boolean(icon));
    const buttonClassName = getButtonClassName({
      autoInsertSpaceInButton,
      block,
      buttonType,
      className,
      ghost,
      hasTwoCNChar,
      iconOnly,
      loading: visualLoading,
      prefixCls,
      shape,
      size,
    });

    useEffect(() => {
      let loadingTimer: number | undefined;
      if (isDelayedLoading(loading)) {
        loadingTimer = window.setTimeout(() => setLoadingState(loading), loading.delay);
      } else {
        setLoadingState(loading ?? false);
      }
      return () => {
        if (loadingTimer !== undefined) window.clearTimeout(loadingTimer);
      };
    }, [loading]);

    useEffect(() => {
      const text = nodeRef.current?.textContent?.replace(/\s/g, '') ?? '';
      setHasTwoCNChar(needInserted && TWO_CN_CHAR.test(text));
    }, [children, needInserted]);

    const handleClick = (event: MouseEvent<HTMLElement>) => {
      if (clickLoading) return;
      if (buttonType !== 'link') triggerLegacyWave(event.currentTarget);
      onClick?.(event);
    };

    if (rest.href !== undefined) {
      const { disabled: _disabled, ...anchorRest } = rest;
      return (
        <a
          {...(anchorRest as AnchorHTMLAttributes<HTMLAnchorElement>)}
          ref={mergeRefs(
            ref as Ref<HTMLAnchorElement>,
            nodeRef as { current: HTMLAnchorElement | null },
          )}
          className={buttonClassName}
          style={style}
          onClick={handleClick}
        >
          {buttonIcon}
          {content}
        </a>
      );
    }

    return (
      <button
        ref={mergeRefs(
          ref as Ref<HTMLButtonElement>,
          nodeRef as { current: HTMLButtonElement | null },
        )}
        type={htmlType ?? (isNativeButtonType(type) ? type : 'button')}
        className={buttonClassName}
        style={style}
        {...rest}
        onClick={handleClick}
      >
        {buttonIcon}
        {content}
      </button>
    );
  },
);

function LegacyButtonGroup({
  children,
  className,
  prefixCls = 'ant-btn-group',
  size,
  ...rest
}: LegacyButtonGroupProps) {
  const sizeClassName = getSizeClassName(prefixCls, size);
  const groupClassName = [prefixCls, sizeClassName, className].filter(Boolean).join(' ');

  return (
    <div {...rest} className={groupClassName}>
      {children}
    </div>
  );
}

export const LegacyButton = Object.assign(LegacyButtonBase, {
  Group: LegacyButtonGroup,
});

(LegacyButton as typeof LegacyButton & { __ANT_BUTTON: boolean }).__ANT_BUTTON = true;
