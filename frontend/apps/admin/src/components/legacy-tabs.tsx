import {
  Children,
  isValidElement,
  cloneElement,
  useLayoutEffect,
  useMemo,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';

type LegacyTabsAnimated = boolean | { inkBar?: boolean; tabPane?: boolean };

interface LegacyTabsProps {
  activeKey?: string;
  animated?: LegacyTabsAnimated;
  children: ReactNode;
  className?: string;
  defaultActiveKey?: string;
  destroyInactiveTabPane?: boolean;
  onChange?: (key: string) => void;
  onTabClick?: (key: string) => void;
  prefixCls?: string;
  size?: 'large' | 'default' | 'small';
  style?: CSSProperties;
  tabBarExtraContent?: ReactNode;
  tabBarStyle?: CSSProperties;
  tabPosition?: 'top' | 'bottom' | 'left' | 'right';
  type?: 'line' | 'card' | 'editable-card';
}

interface LegacyTabPaneProps {
  children?: ReactNode;
  className?: string;
  disabled?: boolean;
  forceRender?: boolean;
  id?: string;
  placeholder?: ReactNode;
  style?: CSSProperties;
  tab: ReactNode;
}

interface PaneItem {
  children: ReactNode;
  className?: string;
  disabled: boolean;
  forceRender: boolean;
  id?: string;
  key: string;
  placeholder?: ReactNode;
  style?: CSSProperties;
  tab: ReactNode;
}

function LegacyTabPane(_props: LegacyTabPaneProps) {
  return null;
}

function collectPanes(children: ReactNode): PaneItem[] {
  const panes: PaneItem[] = [];
  Children.forEach(children, (child) => {
    if (!isValidElement<LegacyTabPaneProps>(child)) return;
    const key = child.key === null ? String(panes.length) : String(child.key);
    panes.push({
      children: child.props.children,
      className: child.props.className,
      disabled: !!child.props.disabled,
      forceRender: !!child.props.forceRender,
      id: child.props.id,
      key,
      placeholder: child.props.placeholder,
      style: child.props.style,
      tab: child.props.tab,
    });
  });
  return panes;
}

function mergeClassName(...values: Array<string | undefined | false>) {
  return values.filter(Boolean).join(' ');
}

function firstEnabledKey(panes: PaneItem[]) {
  return panes.find((pane) => !pane.disabled)?.key;
}

function hasPaneKey(panes: PaneItem[], key: string) {
  return panes.some((pane) => pane.key === key);
}

function getNextActiveKey(panes: PaneItem[], activeKey: string, forward: boolean) {
  const enabledPanes = panes.filter((pane) => !pane.disabled);
  if (enabledPanes.length === 0) return activeKey;
  const activeIndex = enabledPanes.findIndex((pane) => pane.key === activeKey);
  if (activeIndex === -1) {
    return enabledPanes[0]!.key;
  }
  const offset = forward ? 1 : -1;
  const nextIndex = (activeIndex + offset + enabledPanes.length) % enabledPanes.length;
  return enabledPanes[nextIndex]!.key;
}

function getAnimatedValue(
  animated: LegacyTabsAnimated | undefined,
  key: 'inkBar' | 'tabPane',
) {
  if (typeof animated === 'object') return Boolean(animated[key]);
  return animated !== false;
}

function LegacyLeftIcon({ prefixCls }: { prefixCls: string }) {
  return (
    <i aria-label="图标: left" className={`anticon anticon-left ${prefixCls}-tab-prev-icon-target`}>
      <svg
        viewBox="64 64 896 896"
        focusable="false"
        className=""
        data-icon="left"
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M724 218.3V141c0-6.7-7.7-10.4-12.9-6.3L260.3 486.8a31.86 31.86 0 0 0 0 50.3l450.8 352.1c5.3 4.1 12.9.4 12.9-6.3v-77.3c0-4.9-2.3-9.6-6.1-12.6l-360-281 360-281.1c3.8-3 6.1-7.7 6.1-12.6z" />
      </svg>
    </i>
  );
}

function LegacyRightIcon({ prefixCls }: { prefixCls: string }) {
  return (
    <i
      aria-label="图标: right"
      className={`anticon anticon-right ${prefixCls}-tab-next-icon-target`}
    >
      <svg
        viewBox="64 64 896 896"
        focusable="false"
        className=""
        data-icon="right"
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M765.7 486.8L314.9 134.7A7.97 7.97 0 0 0 302 141v77.3c0 4.9 2.3 9.6 6.1 12.6l360 281.1-360 281.1c-3.9 3-6.1 7.7-6.1 12.6V883c0 6.7 7.7 10.4 12.9 6.3l450.8-352.1a31.96 31.96 0 0 0 0-50.4z" />
      </svg>
    </i>
  );
}

function LegacyTabsComponent(props: LegacyTabsProps) {
  const {
    activeKey,
    animated,
    children,
    className,
    defaultActiveKey,
    destroyInactiveTabPane = false,
    onChange,
    onTabClick,
    prefixCls = 'ant-tabs',
    size,
    style,
    tabBarExtraContent,
    tabBarStyle,
    tabPosition = 'top',
    type = 'line',
  } = props;
  const panes = useMemo(() => collectPanes(children), [children]);
  const fallbackKey = firstEnabledKey(panes) ?? '';
  const initialActiveKey = activeKey ?? defaultActiveKey ?? fallbackKey;
  const isControlled = Object.prototype.hasOwnProperty.call(props, 'activeKey');
  const [innerActiveKey, setInnerActiveKey] = useState(initialActiveKey);
  const actualActiveKey = isControlled ? (activeKey ?? '') : innerActiveKey;
  const [visitedKeys, setVisitedKeys] = useState(() => new Set([initialActiveKey]));
  const tabRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const activeIndex = Math.max(
    0,
    panes.findIndex((pane) => pane.key === actualActiveKey),
  );
  const [inkStyle, setInkStyle] = useState({ left: 0, width: 0 });
  const hasAnimatedProp = Object.prototype.hasOwnProperty.call(props, 'animated');
  const inkBarAnimated = getAnimatedValue(animated, 'inkBar');
  const tabPaneAnimated =
    type === 'line'
      ? getAnimatedValue(animated, 'tabPane')
      : hasAnimatedProp && getAnimatedValue(animated, 'tabPane');
  const isCardType = type.includes('card');
  const horizontalTabBar = tabPosition === 'top' || tabPosition === 'bottom';

  useEffect(() => {
    if (isControlled || hasPaneKey(panes, innerActiveKey)) return;
    setInnerActiveKey(fallbackKey);
    setVisitedKeys((previous) => {
      const next = new Set(previous);
      next.add(fallbackKey);
      return next;
    });
  }, [fallbackKey, innerActiveKey, isControlled, panes]);

  useEffect(() => {
    if (!actualActiveKey) return;
    setVisitedKeys((previous) => {
      if (previous.has(actualActiveKey)) return previous;
      const next = new Set(previous);
      next.add(actualActiveKey);
      return next;
    });
  }, [actualActiveKey]);

  useLayoutEffect(() => {
    const tab = tabRefs.current[actualActiveKey];
    if (!tab) return;
    setInkStyle({ left: tab.offsetLeft, width: tab.offsetWidth });
  }, [actualActiveKey, panes]);

  const activate = (key: string) => {
    const pane = panes.find((item) => item.key === key);
    if (!pane || pane.disabled || key === actualActiveKey) return;
    if (!isControlled) setInnerActiveKey(key);
    setVisitedKeys((previous) => {
      const next = new Set(previous);
      next.add(key);
      return next;
    });
    onChange?.(key);
  };

  const handleTabClick = (key: string) => {
    const pane = panes.find((item) => item.key === key);
    if (!pane || pane.disabled) return;
    onTabClick?.(key);
    activate(key);
  };

  const handleTabBarKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const forward =
      event.key === 'ArrowRight' ||
      event.key === 'ArrowDown' ||
      event.keyCode === 39 ||
      event.keyCode === 40;
    const backward =
      event.key === 'ArrowLeft' ||
      event.key === 'ArrowUp' ||
      event.keyCode === 37 ||
      event.keyCode === 38;

    if (!forward && !backward) return;
    event.preventDefault();
    activate(getNextActiveKey(panes, actualActiveKey, forward));
  };

  const tabsClassName = mergeClassName(
    prefixCls,
    `${prefixCls}-${tabPosition}`,
    (tabPosition === 'left' || tabPosition === 'right') && `${prefixCls}-vertical`,
    size && `${prefixCls}-${size}`,
    isCardType && `${prefixCls}-card`,
    `${prefixCls}-${type}`,
    !tabPaneAnimated && `${prefixCls}-no-animation`,
    className,
  );
  const tabsBarClassName = mergeClassName(
    `${prefixCls}-bar`,
    `${prefixCls}-${tabPosition}-bar`,
    size && `${prefixCls}-${size}-bar`,
    isCardType && `${prefixCls}-card-bar`,
  );
  const tabsContentClassName = mergeClassName(
    `${prefixCls}-content`,
    tabPaneAnimated ? `${prefixCls}-content-animated` : `${prefixCls}-content-no-animated`,
    `${prefixCls}-${tabPosition}-content`,
    isCardType && `${prefixCls}-card-content`,
  );
  const contentStyle = tabPaneAnimated ? { marginLeft: `${-activeIndex * 100}%` } : undefined;
  const extraContent =
    tabBarExtraContent !== undefined && tabBarExtraContent !== null ? (
      <div
        className={`${prefixCls}-extra-content`}
        style={horizontalTabBar ? { float: 'right' } : undefined}
      >
        {tabBarExtraContent}
      </div>
    ) : null;
  const tabBarContent = (
    <div className={`${prefixCls}-nav-container`}>
      <span
        unselectable={'unselectable' as 'on'}
        className={`${prefixCls}-tab-prev ${prefixCls}-tab-btn-disabled`}
      >
        <span className={`${prefixCls}-tab-prev-icon`}>
          <LegacyLeftIcon prefixCls={prefixCls} />
        </span>
      </span>
      <span
        unselectable={'unselectable' as 'on'}
        className={`${prefixCls}-tab-next ${prefixCls}-tab-btn-disabled`}
      >
        <span className={`${prefixCls}-tab-next-icon`}>
          <LegacyRightIcon prefixCls={prefixCls} />
        </span>
      </span>
      <div className={`${prefixCls}-nav-wrap`}>
        <div className={`${prefixCls}-nav-scroll`}>
          <div
            className={mergeClassName(
              `${prefixCls}-nav`,
              inkBarAnimated
                ? `${prefixCls}-nav-animated`
                : `${prefixCls}-nav-no-animated`,
            )}
          >
            <div>
              {panes.map((pane) => {
                const active = pane.key === actualActiveKey;
                return (
                  <div
                    key={pane.key}
                    ref={(element) => {
                      tabRefs.current[pane.key] = element;
                    }}
                    role="tab"
                    aria-disabled={pane.disabled ? 'true' : 'false'}
                    aria-selected={active}
                    className={mergeClassName(
                      active && `${prefixCls}-tab-active`,
                      `${active ? '' : ' '}${prefixCls}-tab`,
                      pane.disabled && `${prefixCls}-tab-disabled`,
                    )}
                    onClick={pane.disabled ? undefined : () => handleTabClick(pane.key)}
                  >
                    {pane.tab}
                  </div>
                );
              })}
            </div>
            <div
              className={mergeClassName(
                `${prefixCls}-ink-bar`,
                inkBarAnimated
                  ? `${prefixCls}-ink-bar-animated`
                  : `${prefixCls}-ink-bar-no-animated`,
              )}
              style={{
                display: 'block',
                transform: `translate3d(${inkStyle.left}px, 0px, 0px)`,
                width: inkStyle.width,
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
  const tabBarChildren =
    extraContent && horizontalTabBar
      ? [
          cloneElement(extraContent, { key: 'extra' }),
          cloneElement(tabBarContent, { key: 'content' }),
        ]
      : extraContent
        ? [
            cloneElement(tabBarContent, { key: 'content' }),
            cloneElement(extraContent, { key: 'extra' }),
          ]
        : tabBarContent;

  return (
    <div className={tabsClassName} style={style}>
      <div
        role="tablist"
        className={tabsBarClassName}
        onKeyDown={handleTabBarKeyDown}
        tabIndex={0}
        style={tabBarStyle}
      >
        {tabBarChildren}
      </div>
      <div
        tabIndex={0}
        role="presentation"
        style={{ width: 0, height: 0, overflow: 'hidden', position: 'absolute' }}
      />
      <div
        className={tabsContentClassName}
        style={contentStyle}
      >
        {panes.map((pane) => {
          const active = pane.key === actualActiveKey;
          const hasVisited = active || visitedKeys.has(pane.key);
          const shouldRender = destroyInactiveTabPane
            ? active || pane.forceRender
            : hasVisited || pane.forceRender;
          return (
            <div
              key={pane.key}
              id={pane.id}
              role="tabpanel"
              aria-hidden={!active}
              className={mergeClassName(
                `${prefixCls}-tabpane`,
                active ? `${prefixCls}-tabpane-active` : `${prefixCls}-tabpane-inactive`,
                pane.className,
              )}
              style={pane.style}
            >
              {active ? (
                <div
                  tabIndex={0}
                  role="presentation"
                  style={{ width: 0, height: 0, overflow: 'hidden', position: 'absolute' }}
                />
              ) : null}
              {shouldRender ? pane.children : pane.placeholder ?? null}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export const LegacyTabs = LegacyTabsComponent as typeof LegacyTabsComponent & {
  TabPane: (props: LegacyTabPaneProps) => ReactElement | null;
};

LegacyTabs.TabPane = LegacyTabPane;
