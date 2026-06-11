import type { HTMLAttributes, ReactNode } from 'react';

type LegacyDividerType = 'horizontal' | 'vertical';
type LegacyDividerOrientation = '' | 'left' | 'right' | 'center';

interface LegacyDividerProps extends HTMLAttributes<HTMLDivElement> {
  children?: ReactNode;
  dashed?: boolean;
  orientation?: LegacyDividerOrientation;
  prefixCls?: string;
  type?: LegacyDividerType;
}

function mergeClassName(...tokens: Array<string | false | null | undefined>) {
  return tokens.filter(Boolean).join(' ');
}

export function LegacyDivider({
  children,
  className,
  dashed,
  orientation = 'center',
  prefixCls = 'ant-divider',
  type = 'horizontal',
  ...restProps
}: LegacyDividerProps) {
  const hasChildren = Boolean(children);
  const orientationSuffix = orientation.length > 0 ? `-${orientation}` : orientation;
  const dividerClassName = mergeClassName(
    className,
    prefixCls,
    `${prefixCls}-${type}`,
    hasChildren ? `${prefixCls}-with-text${orientationSuffix}` : null,
    dashed ? `${prefixCls}-dashed` : null,
  );

  return (
    <div {...restProps} className={dividerClassName} role="separator">
      {hasChildren ? <span className={`${prefixCls}-inner-text`}>{children}</span> : null}
    </div>
  );
}
