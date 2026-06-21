import {
  Children,
  cloneElement,
  isValidElement,
  useEffect,
  useRef,
  useState,
  type HTMLAttributes,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { useTransitionStatus } from '@/lib/use-transition-status';

// antd v3 Tooltip (rc-tooltip) portals its overlay to document.body so it escapes the
// table body's `overflow:hidden` clip, and opens/closes with a 0.1s delay. The rewrite's
// table headers live inside `.ant-table-body{overflow-y:hidden!important}`, where a pure-CSS
// ::after tooltip would be clipped — so reproduce the portal here. The overlay is
// `.ant-tooltip > (.ant-tooltip-arrow, .ant-tooltip-inner[role=tooltip])` — there is no
// `-content` wrapper in this antd build; rc-tooltip returns [arrow, inner]. The
// deployed theme already styles `.ant-tooltip*`, so reusing those classes inherits
// the original's dark bubble + arrow.
export function LegacyTooltip({
  title,
  children,
  placement = 'top',
}: {
  title: ReactNode;
  children: ReactNode;
  // antd v3 Tooltip defaults to `top` (centered); the traffic total column — the
  // fixed-right edge column — uses `topRight` so its bubble right-aligns instead of
  // overflowing off-screen.
  placement?: 'top' | 'topRight';
}) {
  const enterTimer = useRef<number | undefined>(undefined);
  const leaveTimer = useRef<number | undefined>(undefined);
  const [box, setBox] = useState<{ top: number; left: number } | null>(null);
  const [visible, setVisible] = useState(false);
  // antd Tooltip uses transitionName="zoom-big-fast": css-animation waits 30ms
  // before adding "-active", then the CSS animation itself runs for 0.1s.
  const motionStatus = useTransitionStatus(visible, 130, 30);
  const hasTitle = title !== null && title !== undefined && title !== '';

  useEffect(() => {
    if (motionStatus === 'exited') setBox(null);
  }, [motionStatus]);

  useEffect(
    () => () => {
      window.clearTimeout(enterTimer.current);
      window.clearTimeout(leaveTimer.current);
    },
    [],
  );

  const show = (event: MouseEvent) => {
    if (!hasTitle) return;
    const trigger = event.currentTarget as HTMLElement;
    window.clearTimeout(leaveTimer.current);
    enterTimer.current = window.setTimeout(() => {
      const rect = trigger.getBoundingClientRect();
      setBox({
        top: rect.top + window.scrollY - 4,
        left:
          (placement === 'topRight' ? rect.right : rect.left + rect.width / 2) +
          window.scrollX,
      });
      setVisible(true);
    }, 100);
  };

  const hide = () => {
    window.clearTimeout(enterTimer.current);
    leaveTimer.current = window.setTimeout(() => setVisible(false), 100);
  };

  const open = visible && box !== null;
  const motionClass =
    motionStatus === 'enter'
      ? 'zoom-big-fast-enter'
      : motionStatus === 'entering'
        ? 'zoom-big-fast-enter zoom-big-fast-enter-active'
        : motionStatus === 'leave'
          ? 'zoom-big-fast-leave'
          : motionStatus === 'leaving'
            ? 'zoom-big-fast-leave zoom-big-fast-leave-active'
            : '';
  const handlers = { onMouseEnter: show, onMouseLeave: hide };

  const overlay =
    box &&
    motionStatus !== 'exited' &&
    createPortal(
      // The popup wrapper class is `"ant-tooltip" + " " + "" + " " + placementClass`; the
      // empty middle token leaves a double space, reproduced verbatim.
      <div
        className={`ant-tooltip  ant-tooltip-placement-${placement}${motionClass ? ` ${motionClass}` : ''}`}
        style={{
          position: 'absolute',
          top: box.top,
          left: box.left,
          translate: placement === 'topRight' ? '-100% -100%' : '-50% -100%',
          transformOrigin:
            placement === 'topRight' ? '100% calc(100% + 4px)' : '50% calc(100% + 4px)',
          zIndex: 1060,
        }}
      >
        <div className="ant-tooltip-arrow" />
        <div className="ant-tooltip-inner" role="tooltip">
          {title}
        </div>
      </div>,
      document.body,
    );

  // antd renders `visible ? cloneElement(child, {className: classNames(child.props.className,
  // 'ant-tooltip-open')}) : child` — a single valid element is shown directly (no wrapper), and
  // multi-child / text content is wrapped in a style-less span. We
  // always clone to attach the hover handlers the original gets from its rc-trigger wrapper, but
  // only fold in `ant-tooltip-open` while open, leaving the child's className verbatim when closed
  // (so no trailing space and no empty `class=""`).
  const child = Children.count(children) === 1 ? Children.only(children) : null;
  if (isValidElement(child)) {
    const element = child as ReactElement<HTMLAttributes<HTMLElement>>;
    const className = open
      ? [element.props.className, 'ant-tooltip-open'].filter(Boolean).join(' ')
      : element.props.className;
    return (
      <>
        {cloneElement(element, { ...handlers, className })}
        {overlay}
      </>
    );
  }

  return (
    <>
      <span {...handlers} className={open ? 'ant-tooltip-open' : undefined}>
        {children}
      </span>
      {overlay}
    </>
  );
}
