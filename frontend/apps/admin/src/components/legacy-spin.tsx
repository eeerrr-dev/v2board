import {
  cloneElement,
  isValidElement,
  useEffect,
  useState,
  type CSSProperties,
  type HTMLAttributes,
  type ReactElement,
  type ReactNode,
} from 'react';

type LegacySpinSize = 'small' | 'default' | 'large';

interface LegacySpinProps extends Omit<HTMLAttributes<HTMLDivElement>, 'children'> {
  children?: ReactNode;
  delay?: number;
  indicator?: ReactElement<{ className?: string }> | null;
  loading?: boolean;
  prefixCls?: string;
  size?: LegacySpinSize;
  spinning?: boolean;
  style?: CSSProperties;
  tip?: ReactNode;
  wrapperClassName?: string;
}

function mergeClassName(...values: Array<string | undefined | false>) {
  return values.filter(Boolean).join(' ');
}

function shouldDelaySpin(spinning: boolean, delay: number | undefined) {
  return !!spinning && !!delay && !Number.isNaN(Number(delay));
}

function renderIndicator(prefixCls: string, indicator: LegacySpinProps['indicator']) {
  const dotClassName = `${prefixCls}-dot`;

  if (indicator === null) return null;
  if (isValidElement<{ className?: string }>(indicator)) {
    return cloneElement(indicator, {
      className: mergeClassName(indicator.props.className, dotClassName),
    });
  }

  return (
    <span className={`${dotClassName} ${prefixCls}-dot-spin`}>
      <i className={`${dotClassName}-item`} />
      <i className={`${dotClassName}-item`} />
      <i className={`${dotClassName}-item`} />
      <i className={`${dotClassName}-item`} />
    </span>
  );
}

export function LegacySpin({
  children,
  className,
  delay,
  indicator = <div className="spinner-grow text-primary" />,
  loading,
  prefixCls = 'ant-spin',
  size = 'default',
  spinning: spinningProp,
  style,
  tip,
  wrapperClassName = '',
  ...restProps
}: LegacySpinProps) {
  const spinning = spinningProp ?? loading ?? true;
  const [activeSpinning, setActiveSpinning] = useState(
    () => spinning && !shouldDelaySpin(spinning, delay),
  );
  const spinClassName = mergeClassName(
    prefixCls,
    size === 'small' && `${prefixCls}-sm`,
    size === 'large' && `${prefixCls}-lg`,
    activeSpinning && `${prefixCls}-spinning`,
    !!tip && `${prefixCls}-show-text`,
    className,
  );
  const spinElement = (
    <div {...restProps} style={style} className={spinClassName}>
      {renderIndicator(prefixCls, indicator)}
      {tip ? <div className={`${prefixCls}-text`}>{tip}</div> : null}
    </div>
  );

  useEffect(() => {
    if (!spinning) {
      setActiveSpinning(false);
      return undefined;
    }

    if (!shouldDelaySpin(spinning, delay)) {
      setActiveSpinning(true);
      return undefined;
    }

    const timer = window.setTimeout(() => setActiveSpinning(true), Number(delay));
    return () => window.clearTimeout(timer);
  }, [delay, spinning]);

  if (!children) return spinElement;

  return (
    <div
      {...restProps}
      className={mergeClassName(`${prefixCls}-nested-loading`, wrapperClassName)}
    >
      {activeSpinning ? <div>{spinElement}</div> : null}
      <div
        className={mergeClassName(`${prefixCls}-container`, activeSpinning && `${prefixCls}-blur`)}
      >
        {children}
      </div>
    </div>
  );
}
