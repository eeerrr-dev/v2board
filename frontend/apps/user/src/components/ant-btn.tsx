import {
  Children,
  cloneElement,
  isValidElement,
  type ButtonHTMLAttributes,
  type ReactElement,
  type ReactNode,
} from 'react';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';

const TWO_CN_CHAR = /^[一-龥]{2}$/;

function isStringOrNumber(value: ReactNode): value is string | number {
  return typeof value === 'string' || typeof value === 'number';
}

function isStringElement(
  value: ReactNode,
): value is ReactElement<{ children?: ReactNode }, string> {
  return isValidElement(value) && typeof value.type === 'string';
}

function countsForInsertedSpace(value: ReactNode) {
  return (
    value !== null &&
    value !== undefined &&
    typeof value !== 'boolean' &&
    !(isValidElement(value) && value.type === LegacyLoadingIcon)
  );
}

// Mirrors Ant Design's autoInsertSpaceInButton: string children are wrapped in a
// <span>, and a single all-Chinese two-character label gets a space inserted
// between the two characters (e.g. "充值" → "充 值").
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
    if (countsForInsertedSpace(child)) count += 1;
  });
  return count === 1;
}

// Mirrors antd's Wave: on click, derive the ripple colour from the button's own
// border/background (greys/white/transparent fall back to the global
// --antd-wave-shadow-color, exactly like antd's isNotGrey check), then toggle
// ant-click-animating-without-extra-node to (re)start the CSS ripple.
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
  void node.offsetWidth; // force reflow so a rapid re-click restarts the animation
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

export function AntBtn({
  children,
  className,
  onClick,
  type = 'button',
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement>) {
  // autoInsertSpaceInButton is on in this build, so the inserted space makes
  // antd's fixTwoCNChar regex fail and the .ant-btn-two-chinese-chars class is
  // never applied — match that by leaving className untouched.
  const needInserted = shouldInsertSpace(children);
  return (
    <button
      {...rest}
      type={type}
      className={className || undefined}
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
}
