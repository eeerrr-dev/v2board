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

// antd v3 Tooltip portals `.ant-tooltip > (.ant-tooltip-arrow, .ant-tooltip-inner)`
// directly to document.body and opens/closes through rc-trigger's 0.1s mouse delays.
// The admin shell already loads the bundled theme CSS, so keeping the legacy classes
// preserves the original bubble, arrow, and zoom-big-fast animation.
export function LegacyTooltip({
  title,
  children,
  placement = 'top',
}: {
  title: ReactNode;
  children: ReactNode;
  placement?: 'top' | 'topRight';
}) {
  const enterTimer = useRef<number | undefined>(undefined);
  const leaveTimer = useRef<number | undefined>(undefined);
  const [box, setBox] = useState<{ top: number; left: number } | null>(null);
  const [visible, setVisible] = useState(false);
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
        left: (placement === 'topRight' ? rect.right : rect.left + rect.width / 2) + window.scrollX,
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
