import {
  Children,
  cloneElement,
  isValidElement,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type FocusEvent,
  type HTMLAttributes,
  type MutableRefObject,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
} from 'react';
import { createPortal } from 'react-dom';
import { useTransitionStatus } from '@/lib/use-transition-status';

type LegacyTooltipPlacement =
  | 'top'
  | 'topLeft'
  | 'topRight'
  | 'bottom'
  | 'bottomLeft'
  | 'bottomRight'
  | 'left'
  | 'leftTop'
  | 'leftBottom'
  | 'right'
  | 'rightTop'
  | 'rightBottom';

type LegacyTooltipTrigger = 'hover' | 'click' | 'focus' | 'contextMenu';

type LegacyTooltipProps = {
  title?: ReactNode;
  overlay?: ReactNode | (() => ReactNode);
  children: ReactNode;
  mouseEnterDelay?: number;
  mouseLeaveDelay?: number;
  placement?: LegacyTooltipPlacement;
  visible?: boolean;
  defaultVisible?: boolean;
  onVisibleChange?: (visible: boolean) => void;
  afterVisibleChange?: (visible: boolean) => void;
  overlayClassName?: string;
  overlayStyle?: CSSProperties;
  getPopupContainer?: (triggerNode: HTMLElement) => HTMLElement | null;
  getTooltipContainer?: (triggerNode: HTMLElement) => HTMLElement | null;
  openClassName?: string;
  prefixCls?: string;
  trigger?: LegacyTooltipTrigger | LegacyTooltipTrigger[];
  destroyTooltipOnHide?: boolean;
  id?: string;
  arrowContent?: ReactNode;
  transitionName?: string;
  animation?: string;
  align?: unknown;
  arrowPointAtCenter?: boolean;
  autoAdjustOverflow?: boolean;
};

type TooltipElementProps = HTMLAttributes<HTMLElement> & {
  disabled?: boolean;
  style?: CSSProperties;
};

type LegacyDisabledTooltipTarget = {
  __ANT_BUTTON?: boolean;
  __ANT_CHECKBOX?: boolean;
  __ANT_SWITCH?: boolean;
};

type TooltipPopupState = {
  top: number;
  left: number;
  container: HTMLElement;
};

function mergeClassNames(...classes: Array<string | false | null | undefined>) {
  return classes.filter(Boolean).join(' ');
}

function getTooltipBox(placement: LegacyTooltipPlacement, rect: DOMRect) {
  if (placement.startsWith('left')) {
    return {
      top:
        (placement === 'leftTop'
          ? rect.top
          : placement === 'leftBottom'
            ? rect.bottom
            : rect.top + rect.height / 2) + window.scrollY,
      left: rect.left + window.scrollX - 4,
    };
  }
  if (placement.startsWith('right')) {
    return {
      top:
        (placement === 'rightTop'
          ? rect.top
          : placement === 'rightBottom'
            ? rect.bottom
            : rect.top + rect.height / 2) + window.scrollY,
      left: rect.right + window.scrollX + 4,
    };
  }
  if (placement.startsWith('bottom')) {
    return {
      top: rect.bottom + window.scrollY + 4,
      left:
        (placement === 'bottomLeft'
          ? rect.left
          : placement === 'bottomRight'
            ? rect.right
            : rect.left + rect.width / 2) + window.scrollX,
    };
  }

  return {
    top: rect.top + window.scrollY - 4,
    left:
      (placement === 'topLeft'
        ? rect.left
        : placement === 'topRight'
          ? rect.right
          : rect.left + rect.width / 2) + window.scrollX,
  };
}

function getTooltipTranslate(placement: LegacyTooltipPlacement) {
  if (placement === 'left') return '-100% -50%';
  if (placement === 'leftTop') return '-100% 0';
  if (placement === 'leftBottom') return '-100% -100%';
  if (placement === 'right') return '0 -50%';
  if (placement === 'rightTop') return '0 0';
  if (placement === 'rightBottom') return '0 -100%';
  if (placement === 'topLeft') return '0 -100%';
  if (placement === 'topRight') return '-100% -100%';
  if (placement === 'bottomLeft') return '0 0';
  if (placement === 'bottomRight') return '-100% 0';
  return placement === 'bottom' ? '-50% 0' : '-50% -100%';
}

function getTooltipTransformOrigin(placement: LegacyTooltipPlacement) {
  if (placement === 'left') return 'calc(100% + 4px) 50%';
  if (placement === 'leftTop') return 'calc(100% + 4px) 0';
  if (placement === 'leftBottom') return 'calc(100% + 4px) 100%';
  if (placement === 'right') return '-4px 50%';
  if (placement === 'rightTop') return '-4px 0';
  if (placement === 'rightBottom') return '-4px 100%';
  if (placement === 'topLeft') return '0 calc(100% + 4px)';
  if (placement === 'topRight') return '100% calc(100% + 4px)';
  if (placement === 'bottomLeft') return '0 -4px';
  if (placement === 'bottomRight') return '100% -4px';
  return placement === 'bottom' ? '50% -4px' : '50% calc(100% + 4px)';
}

function assignRef<T>(ref: Ref<T> | undefined, node: T | null) {
  if (!ref) return;
  if (typeof ref === 'function') {
    ref(node);
    return;
  }
  (ref as MutableRefObject<T | null>).current = node;
}

function getElementRef(element: ReactElement): Ref<HTMLElement> | undefined {
  return (element as ReactElement & { ref?: Ref<HTMLElement> }).ref;
}

function isLegacyMarkedDisabledTarget(element: ReactElement<TooltipElementProps>) {
  if (!element.props.disabled || typeof element.type === 'string') return false;
  const type = element.type as LegacyDisabledTooltipTarget;
  return Boolean(type.__ANT_BUTTON || type.__ANT_CHECKBOX || type.__ANT_SWITCH);
}

function shouldWrapDisabledElement(element: ReactElement<TooltipElementProps>) {
  if (!element.props.disabled) return false;
  if (isLegacyMarkedDisabledTarget(element)) return true;
  return (
    typeof element.type === 'string' &&
    ['button', 'input', 'textarea', 'select'].includes(element.type)
  );
}

function wrapDisabledElement(element: ReactElement<TooltipElementProps>) {
  const childStyle = element.props.style ?? {};
  const block = childStyle.display === 'block';
  const wrapperStyle: CSSProperties = {
    display: block ? 'block' : undefined,
    cursor: 'not-allowed',
    width: block ? '100%' : undefined,
  };

  return (
    <span className={element.props.className} style={wrapperStyle}>
      {cloneElement(element, {
        className: undefined,
        style: {
          ...childStyle,
          pointerEvents: 'none',
        },
      })}
    </span>
  ) as ReactElement<TooltipElementProps>;
}

// antd v3 Tooltip portals `.ant-tooltip > (.ant-tooltip-arrow, .ant-tooltip-inner)`
// directly to document.body and follows rc-trigger's 0.1s hover enter/leave delay.
// The admin shell loads source-owned Ant Design v3 parity styles, so keeping the
// legacy classes preserves the original bubble, arrow, and zoom-big-fast animation.
export function LegacyTooltip(props: LegacyTooltipProps) {
  const {
    title,
    overlay: overlayProp,
    children,
    mouseEnterDelay = 0.1,
    mouseLeaveDelay = 0.1,
    placement = 'top',
    visible: controlledVisible,
    defaultVisible,
    onVisibleChange,
    afterVisibleChange,
    overlayClassName,
    overlayStyle,
    getPopupContainer,
    getTooltipContainer,
    openClassName,
    prefixCls = 'ant-tooltip',
    trigger = ['hover'],
    destroyTooltipOnHide = false,
    id,
    arrowContent = null,
    transitionName = 'zoom-big-fast',
    animation,
  } = props;
  const enterTimer = useRef<number | undefined>(undefined);
  const leaveTimer = useRef<number | undefined>(undefined);
  const triggerRef = useRef<HTMLElement | null>(null);
  const afterVisibleRef = useRef<boolean | null>(null);
  const hasControlledVisible = Object.prototype.hasOwnProperty.call(props, 'visible');
  const [popup, setPopup] = useState<TooltipPopupState | null>(null);
  const [innerVisible, setInnerVisible] = useState(
    () => !!controlledVisible || !!defaultVisible,
  );
  const visible = hasControlledVisible ? !!controlledVisible : innerVisible;
  const isNoTitle = !title && !overlayProp && title !== 0;
  const hasOverlay = !isNoTitle;
  const effectiveVisible = visible && (hasOverlay || hasControlledVisible);
  const popupVisible = effectiveVisible && popup !== null;
  const motionStatus = useTransitionStatus(popupVisible, 130, 30);
  const triggerModes = Array.isArray(trigger) ? trigger : [trigger];
  const animationName = transitionName || (animation ? `${prefixCls}-${animation}` : '');

  const setTriggerNode = (node: HTMLElement | null) => {
    triggerRef.current = node;
  };

  const updatePopup = (triggerNode: HTMLElement) => {
    const rect = triggerNode.getBoundingClientRect();
    const container =
      getPopupContainer?.(triggerNode) ?? getTooltipContainer?.(triggerNode) ?? document.body;
    setPopup({
      ...getTooltipBox(placement, rect),
      container,
    });
  };

  useEffect(() => {
    if (hasControlledVisible || hasOverlay || !innerVisible) return;
    setInnerVisible(false);
  }, [hasControlledVisible, hasOverlay, innerVisible]);

  useLayoutEffect(() => {
    if (!effectiveVisible || !triggerRef.current) return;
    updatePopup(triggerRef.current);
  }, [effectiveVisible, placement, getPopupContainer, getTooltipContainer]);

  useEffect(() => {
    if (motionStatus === 'entered' && popupVisible && afterVisibleRef.current !== true) {
      afterVisibleRef.current = true;
      afterVisibleChange?.(true);
      return;
    }
    if (motionStatus !== 'exited' || popupVisible || !popup) return;
    if (afterVisibleRef.current !== false) {
      afterVisibleRef.current = false;
      afterVisibleChange?.(false);
    }
    if (destroyTooltipOnHide) setPopup(null);
  }, [afterVisibleChange, destroyTooltipOnHide, motionStatus, popup, popupVisible]);

  useEffect(
    () => () => {
      window.clearTimeout(enterTimer.current);
      window.clearTimeout(leaveTimer.current);
    },
    [],
  );

  const changeVisible = (nextVisible: boolean, triggerNode = triggerRef.current) => {
    window.clearTimeout(enterTimer.current);
    window.clearTimeout(leaveTimer.current);

    if (nextVisible && triggerNode) updatePopup(triggerNode);

    if (nextVisible && !hasOverlay && !hasControlledVisible) {
      setInnerVisible(false);
      return;
    }

    if (visible === nextVisible) return;
    if (!hasControlledVisible) setInnerVisible(nextVisible);
    if (hasOverlay) onVisibleChange?.(nextVisible);
  };

  const showWithDelay = (triggerNode: HTMLElement) => {
    if (!hasOverlay) return;
    window.clearTimeout(enterTimer.current);
    window.clearTimeout(leaveTimer.current);
    const delay = mouseEnterDelay * 1000;
    if (delay > 0 && !visible) {
      enterTimer.current = window.setTimeout(() => changeVisible(true, triggerNode), delay);
      return;
    }
    changeVisible(true, triggerNode);
  };

  const hideWithDelay = () => {
    window.clearTimeout(enterTimer.current);
    leaveTimer.current = window.setTimeout(
      () => changeVisible(false),
      mouseLeaveDelay * 1000,
    );
  };

  const keepOpen = () => {
    window.clearTimeout(leaveTimer.current);
  };

  const open = effectiveVisible;
  const motionClass =
    !animationName || motionStatus === 'entered' || motionStatus === 'exited'
      ? ''
      : motionStatus === 'enter'
        ? `${animationName}-enter`
        : motionStatus === 'entering'
          ? `${animationName}-enter ${animationName}-enter-active`
          : motionStatus === 'leave'
            ? `${animationName}-leave`
            : `${animationName}-leave ${animationName}-leave-active`;
  const hiddenClass = popupVisible || motionStatus !== 'exited' ? '' : `${prefixCls}-hidden`;
  const overlayContent = title === 0 ? title : overlayProp || title || '';
  const renderedOverlay =
    typeof overlayContent === 'function' ? overlayContent() : overlayContent;
  const openChildClassName = openClassName ?? `${prefixCls}-open`;

  const primitiveHandlers = {
    ref: setTriggerNode,
    onMouseEnter: (event: MouseEvent<HTMLElement>) => {
      if (triggerModes.includes('hover')) showWithDelay(event.currentTarget);
    },
    onMouseLeave: () => {
      if (triggerModes.includes('hover')) hideWithDelay();
    },
    onFocus: (event: FocusEvent<HTMLElement>) => {
      if (triggerModes.includes('focus')) changeVisible(true, event.currentTarget);
    },
    onBlur: () => {
      if (triggerModes.includes('focus')) changeVisible(false);
    },
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (!triggerModes.includes('click')) return;
      event.preventDefault();
      changeVisible(!visible, event.currentTarget);
    },
    onContextMenu: (event: MouseEvent<HTMLElement>) => {
      if (!triggerModes.includes('contextMenu')) return;
      event.preventDefault();
      changeVisible(true, event.currentTarget);
    },
  };

  const overlay =
    popup &&
    (!destroyTooltipOnHide || popupVisible || motionStatus !== 'exited') &&
    createPortal(
      <div
        className={mergeClassNames(
          // rc-trigger joins `prefixCls + " " + popupClassName + " " + placementCls` (double
          // space when rc-tooltip leaves overlayClassName empty) and appends "-hidden" last.
          `${prefixCls} ${overlayClassName ?? ''} ${prefixCls}-placement-${placement}`,
          hiddenClass,
          motionClass,
        )}
        style={{
          position: 'absolute',
          top: popup.top,
          left: popup.left,
          translate: getTooltipTranslate(placement),
          transformOrigin: getTooltipTransformOrigin(placement),
          zIndex: 1060,
          ...overlayStyle,
        }}
        onMouseEnter={keepOpen}
        onMouseLeave={hideWithDelay}
      >
        <div className={`${prefixCls}-arrow`}>{arrowContent}</div>
        <div className={`${prefixCls}-inner`} id={id} role="tooltip">
          {renderedOverlay}
        </div>
      </div>,
      popup.container,
    );

  const child = Children.count(children) === 1 ? Children.toArray(children)[0] : null;
  if (isValidElement(child)) {
    const originalElement = child as ReactElement<TooltipElementProps>;
    const element = shouldWrapDisabledElement(originalElement)
      ? wrapDisabledElement(originalElement)
      : originalElement;
    const className = open
      ? mergeClassNames(element.props.className, openChildClassName)
      : element.props.className;
    const childProps: TooltipElementProps = {
      onMouseEnter: (event: MouseEvent<HTMLElement>) => {
        element.props.onMouseEnter?.(event);
        if (triggerModes.includes('hover')) showWithDelay(event.currentTarget);
      },
      onMouseLeave: (event: MouseEvent<HTMLElement>) => {
        element.props.onMouseLeave?.(event);
        if (triggerModes.includes('hover')) hideWithDelay();
      },
      onFocus: (event: FocusEvent<HTMLElement>) => {
        element.props.onFocus?.(event);
        if (triggerModes.includes('focus')) changeVisible(true, event.currentTarget);
      },
      onBlur: (event: FocusEvent<HTMLElement>) => {
        element.props.onBlur?.(event);
        if (triggerModes.includes('focus')) changeVisible(false);
      },
      onClick: (event: MouseEvent<HTMLElement>) => {
        element.props.onClick?.(event);
        if (!triggerModes.includes('click')) return;
        event.preventDefault();
        changeVisible(!visible, event.currentTarget);
      },
      onContextMenu: (event: MouseEvent<HTMLElement>) => {
        element.props.onContextMenu?.(event);
        if (!triggerModes.includes('contextMenu')) return;
        event.preventDefault();
        changeVisible(true, event.currentTarget);
      },
      className,
    };

    if (typeof element.type === 'string') {
      const originalRef = getElementRef(element);
      (
        childProps as TooltipElementProps & { ref: (node: HTMLElement | null) => void }
      ).ref = (node) => {
        setTriggerNode(node);
        assignRef(originalRef, node);
      };
    }

    return (
      <>
        {cloneElement(element, childProps)}
        {overlay}
      </>
    );
  }

  return (
    <>
      <span {...primitiveHandlers} className={open ? openChildClassName : undefined}>
        {children}
      </span>
      {overlay}
    </>
  );
}
