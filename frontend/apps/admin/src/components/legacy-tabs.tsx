import {
  Children,
  isValidElement,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from 'react';

interface LegacyTabsProps {
  children: ReactNode;
  defaultActiveKey?: string;
  onChange?: (key: string) => void;
  size?: 'large';
}

interface LegacyTabPaneProps {
  children?: ReactNode;
  tab: ReactNode;
}

interface PaneItem {
  children: ReactNode;
  key: string;
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
      key,
      tab: child.props.tab,
    });
  });
  return panes;
}

function LegacyLeftIcon() {
  return (
    <i aria-label="图标: left" className="anticon anticon-left ant-tabs-tab-prev-icon-target">
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

function LegacyRightIcon() {
  return (
    <i aria-label="图标: right" className="anticon anticon-right ant-tabs-tab-next-icon-target">
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

function LegacyTabsComponent({ children, defaultActiveKey, onChange, size }: LegacyTabsProps) {
  const panes = useMemo(() => collectPanes(children), [children]);
  const firstKey = panes[0]?.key ?? '';
  const [activeKey, setActiveKey] = useState(defaultActiveKey ?? firstKey);
  const [visitedKeys, setVisitedKeys] = useState(() => new Set([defaultActiveKey ?? firstKey]));
  const tabRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const activeIndex = Math.max(
    0,
    panes.findIndex((pane) => pane.key === activeKey),
  );
  const [inkStyle, setInkStyle] = useState({ left: 0, width: 0 });

  useLayoutEffect(() => {
    const tab = tabRefs.current[activeKey];
    if (!tab) return;
    setInkStyle({ left: tab.offsetLeft, width: tab.offsetWidth });
  }, [activeKey, panes]);

  const activate = (key: string) => {
    if (key === activeKey) return;
    setActiveKey(key);
    setVisitedKeys((previous) => {
      const next = new Set(previous);
      next.add(key);
      return next;
    });
    onChange?.(key);
  };

  return (
    <div
      className={`ant-tabs ant-tabs-top${size === 'large' ? ' ant-tabs-large' : ''} ant-tabs-line`}
    >
      <div
        role="tablist"
        className={`ant-tabs-bar ant-tabs-top-bar${size === 'large' ? ' ant-tabs-large-bar' : ''}`}
        tabIndex={0}
      >
        <div className="ant-tabs-nav-container">
          <span
            unselectable={'unselectable' as 'on'}
            className="ant-tabs-tab-prev ant-tabs-tab-btn-disabled"
          >
            <span className="ant-tabs-tab-prev-icon">
              <LegacyLeftIcon />
            </span>
          </span>
          <span
            unselectable={'unselectable' as 'on'}
            className="ant-tabs-tab-next ant-tabs-tab-btn-disabled"
          >
            <span className="ant-tabs-tab-next-icon">
              <LegacyRightIcon />
            </span>
          </span>
          <div className="ant-tabs-nav-wrap">
            <div className="ant-tabs-nav-scroll">
              <div className="ant-tabs-nav ant-tabs-nav-animated">
                <div>
                  {panes.map((pane) => {
                    const active = pane.key === activeKey;
                    return (
                      <div
                        key={pane.key}
                        ref={(element) => {
                          tabRefs.current[pane.key] = element;
                        }}
                        role="tab"
                        aria-disabled="false"
                        aria-selected={active}
                        className={active ? 'ant-tabs-tab-active ant-tabs-tab' : ' ant-tabs-tab'}
                        onClick={() => activate(pane.key)}
                      >
                        {pane.tab}
                      </div>
                    );
                  })}
                </div>
                <div
                  className="ant-tabs-ink-bar ant-tabs-ink-bar-animated"
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
      </div>
      <div
        tabIndex={0}
        role="presentation"
        style={{ width: 0, height: 0, overflow: 'hidden', position: 'absolute' }}
      />
      <div
        className="ant-tabs-content ant-tabs-content-animated ant-tabs-top-content"
        style={{ marginLeft: `${-activeIndex * 100}%` }}
      >
        {panes.map((pane) => {
          const active = pane.key === activeKey;
          return (
            <div
              key={pane.key}
              role="tabpanel"
              aria-hidden={!active}
              className={`ant-tabs-tabpane ${
                active ? 'ant-tabs-tabpane-active' : 'ant-tabs-tabpane-inactive'
              }`}
            >
              {active ? (
                <div
                  tabIndex={0}
                  role="presentation"
                  style={{ width: 0, height: 0, overflow: 'hidden', position: 'absolute' }}
                />
              ) : null}
              {active || visitedKeys.has(pane.key) ? pane.children : null}
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
