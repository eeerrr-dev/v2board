import {
  Children,
  cloneElement,
  forwardRef,
  isValidElement,
  type ButtonHTMLAttributes,
  type ReactElement,
  type ReactNode,
} from 'react';

const TWO_CN_CHAR = /^[一-龥]{2}$/;

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

function triggerWave(node: HTMLElement) {
  const style = getComputedStyle(node);
  const waveColor =
    style.getPropertyValue('border-top-color') ||
    style.getPropertyValue('border-color') ||
    style.getPropertyValue('background-color');
  const rgb = waveColor.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  const meaningful =
    !!waveColor &&
    waveColor !== 'transparent' &&
    waveColor !== 'rgb(255, 255, 255)' &&
    !/rgba\((?:\d+,\s*){3}0\)/.test(waveColor) &&
    !(rgb !== null && rgb[1] === rgb[2] && rgb[2] === rgb[3]);
  if (meaningful) node.style.setProperty('--antd-wave-shadow-color', waveColor);

  node.removeAttribute('ant-click-animating-without-extra-node');
  void node.offsetWidth;
  node.setAttribute('ant-click-animating-without-extra-node', 'true');
  const onEnd = (event: AnimationEvent) => {
    if (event.animationName !== 'fadeEffect') return;
    node.removeAttribute('ant-click-animating-without-extra-node');
    node.removeEventListener('animationend', onEnd);
  };
  node.addEventListener('animationend', onEnd);
}

function hasClassToken(className: string | undefined, token: string) {
  return className?.split(/\s+/).includes(token) ?? false;
}

function orderLegacyButtonClassName(className: string | undefined) {
  if (!className) return undefined;
  const tokens = className.split(/\s+/).filter(Boolean);
  if (!tokens.includes('ant-btn')) return className;

  const ordered = [
    ...tokens.filter((token) => token === 'ant-btn'),
    ...tokens.filter((token) => token !== 'ant-btn'),
  ];
  return ordered.join(' ');
}

export const LegacyButton = forwardRef<HTMLButtonElement, ButtonHTMLAttributes<HTMLButtonElement>>(
  function LegacyButton({ children, className, onClick, type = 'button', ...rest }, ref) {
    const needInserted = shouldInsertSpace(children);
    const buttonClassName = orderLegacyButtonClassName(className);
    return (
      <button
        {...rest}
        ref={ref}
        type={type}
        className={buttonClassName}
        onClick={(event) => {
          if (hasClassToken(className, 'ant-btn-loading')) {
            event.preventDefault();
            return;
          }
          triggerWave(event.currentTarget);
          onClick?.(event);
        }}
      >
        {insertSpace(children, needInserted)}
      </button>
    );
  },
);
