import type { CSSProperties, ReactNode } from 'react';

const LEGACY_PRESET_COLORS = new Set([
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
]);

export type LegacyBadgeStatus = 'success' | 'processing' | 'default' | 'error' | 'warning';

export type LegacyBadgeProps = {
  children?: ReactNode;
  className?: string;
  color?: string;
  count?: number | string | ReactNode;
  dot?: boolean;
  offset?: [number | string, number | string];
  overflowCount?: number;
  prefixCls?: string;
  showZero?: boolean;
  status?: LegacyBadgeStatus;
  style?: CSSProperties;
  text?: ReactNode;
  title?: string;
};

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ');
}

function isPresetColor(color: string | undefined) {
  return color !== undefined && LEGACY_PRESET_COLORS.has(color);
}

function isZero(value: ReactNode) {
  return value === 0 || value === '0';
}

function getNumberedDisplayCount(count: LegacyBadgeProps['count'], overflowCount: number) {
  return typeof count === 'number' && count > overflowCount ? `${overflowCount}+` : count;
}

function getDisplayCount({
  count,
  dot,
  hasStatus,
  overflowCount,
}: {
  count: LegacyBadgeProps['count'];
  dot: boolean | undefined;
  hasStatus: boolean;
  overflowCount: number;
}) {
  const numberedDisplayCount = getNumberedDisplayCount(count, overflowCount);
  const badgeIsDot = (dot && !isZero(numberedDisplayCount)) || hasStatus;
  return badgeIsDot ? '' : numberedDisplayCount;
}

function isHidden({
  count,
  dot,
  hasStatus,
  overflowCount,
  showZero,
}: {
  count: LegacyBadgeProps['count'];
  dot: boolean | undefined;
  hasStatus: boolean;
  overflowCount: number;
  showZero: boolean | undefined;
}) {
  const displayCount = getDisplayCount({ count, dot, hasStatus, overflowCount });
  const badgeIsDot = (dot && !isZero(getNumberedDisplayCount(count, overflowCount))) || hasStatus;
  const empty = displayCount === null || displayCount === undefined || displayCount === '';
  return (empty || (isZero(displayCount) && !showZero)) && !badgeIsDot;
}

function getStyleWithOffset(style: CSSProperties | undefined, offset: LegacyBadgeProps['offset']) {
  if (!offset) return style;
  return {
    right: -parseInt(String(offset[0]), 10),
    marginTop: offset[1],
    ...style,
  };
}

export function LegacyBadge({
  children,
  className,
  color,
  count,
  dot,
  offset,
  overflowCount = 99,
  prefixCls = 'ant-badge',
  showZero,
  status,
  style,
  text,
  title,
}: LegacyBadgeProps) {
  const hasStatus = Boolean(status || color);
  const rootClassName = classNames(
    className,
    prefixCls,
    hasStatus && `${prefixCls}-status`,
    !children && `${prefixCls}-not-a-wrapper`,
  );
  const styleWithOffset = getStyleWithOffset(style, offset);
  const dotClassName = classNames(
    `${prefixCls}-status-dot`,
    status && `${prefixCls}-status-${status}`,
    isPresetColor(color) && `${prefixCls}-status-${color}`,
  );
  const dotStyle = color && !isPresetColor(color) ? { background: color } : undefined;

  if (!children && hasStatus) {
    const statusTextColor = styleWithOffset?.color;
    return (
      <span className={rootClassName} style={styleWithOffset} title={title}>
        <span className={dotClassName} style={dotStyle} />
        <span
          style={statusTextColor === undefined ? undefined : { color: statusTextColor }}
          className={`${prefixCls}-status-text`}
        >
          {text}
        </span>
      </span>
    );
  }

  const displayCount = getDisplayCount({ count, dot, hasStatus, overflowCount });
  const hidden = isHidden({ count, dot, hasStatus, overflowCount, showZero });
  const badgeIsDot = (dot && !isZero(getNumberedDisplayCount(count, overflowCount))) || hasStatus;
  const countClassName = classNames(
    badgeIsDot ? `${prefixCls}-dot` : `${prefixCls}-count`,
    !badgeIsDot &&
      count !== null &&
      count !== undefined &&
      count.toString().length > 1 &&
      `${prefixCls}-multiple-words`,
    status && `${prefixCls}-status-${status}`,
    isPresetColor(color) && `${prefixCls}-status-${color}`,
  );
  const countStyle =
    color && !isPresetColor(color)
      ? { ...getStyleWithOffset(undefined, offset), background: color }
      : getStyleWithOffset(undefined, offset);

  return (
    <span className={rootClassName} title={title}>
      {children}
      {hidden ? null : (
        <sup data-show className={countClassName} style={countStyle}>
          {displayCount}
        </sup>
      )}
      {!hidden && text ? <span className={`${prefixCls}-status-text`}>{text}</span> : null}
    </span>
  );
}
