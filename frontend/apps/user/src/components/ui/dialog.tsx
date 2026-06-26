import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '@/lib/cn';
import { AntBtn } from '@/components/ant-btn';
import { CloseIcon } from '@/components/ant-icon';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { lockLegacyModalBodyScroll } from '@/lib/legacy-body-scroll';

// Faithful reproduction of antd v3's Modal (rc-dialog, umi.js). antd Modal builds the
// header/body/footer/close DOM from the title/footer/closable props, so DialogContent does
// the same and the call-sites stay declarative. The rendered tree is
//   <div class="ant-modal-root">
//     <div class="ant-modal-mask">                         (+ fade-* motion / -hidden when closed)
//     <div class="ant-modal-wrap" role="dialog" tabindex="-1">   (+ ant-modal-centered)
//       <div class="ant-modal" role="document">            (+ zoom-* motion, style width)
//         <div tabindex="0" aria-hidden="true" .sentinel>
//         <div class="ant-modal-content">
//           [closable] <button class="ant-modal-close">…</button>
//           [title]    <div class="ant-modal-header"><div class="ant-modal-title">…</div></div>
//           <div class="ant-modal-body">{children}</div>
//           [footer!==null] <div class="ant-modal-footer">…</div>
//         <div tabindex="0" aria-hidden="true" .sentinel>
// Behaviour mirrors rc-dialog: focus-trap via the two sentinels + Tab handler, Esc to close,
// a mask click that is ignored within 300ms of opening or while a drag started on the dialog,
// the zoom transform-origin set from the last click point, and a body scroll-lock.

// rc-dialog reads the last click point (cleared after 100ms) so the modal zooms open from
// where its trigger was clicked: transform-origin = `${x - rect.left}px ${y - rect.top}px`.
let modalMousePosition: { x: number; y: number } | null = null;
if (typeof window !== 'undefined' && window.document?.documentElement) {
  window.document.documentElement.addEventListener('click', (event) => {
    modalMousePosition = { x: event.pageX, y: event.pageY };
    setTimeout(() => {
      modalMousePosition = null;
    }, 100);
  });
}

// rc-dialog's switchScrollingEffect: the first open modal applies the shared scrolling effect
// (ant-scrolling-effect + overflow hidden); the final close restores the previous body styles.
let openModalCount = 0;
let unlockModalBodyScroll: (() => void) | null = null;
function lockBodyScroll() {
  if (openModalCount === 0) {
    unlockModalBodyScroll = lockLegacyModalBodyScroll();
  }
  openModalCount += 1;
}
function unlockBodyScroll() {
  openModalCount = Math.max(0, openModalCount - 1);
  if (openModalCount === 0) {
    unlockModalBodyScroll?.();
    unlockModalBodyScroll = null;
  }
}

interface DialogContextValue {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}
const DialogContext = createContext<DialogContextValue>({ open: false, onOpenChange: () => {} });

export function Dialog({
  open,
  onOpenChange,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}) {
  return <DialogContext.Provider value={{ open, onOpenChange }}>{children}</DialogContext.Provider>;
}

const SENTINEL_STYLE: CSSProperties = { width: 0, height: 0, overflow: 'hidden', outline: 'none' };

// rc-dialog gives every instance a unique title id (`"rcDialogTitle" + b++`) used to wire the
// wrap's aria-labelledby to the header title when a title is present.
let titleIdSeed = 0;

type MotionPhase = 'appear' | 'enter' | 'leave' | null;

interface DialogContentProps {
  // antd Modal props: title renders the header, falsy footer omits it, an undefined
  // footer renders the default Cancel/OK buttons (from okText/cancelText/onOk), and
  // a truthy node renders inside ant-modal-footer verbatim.
  title?: ReactNode;
  footer?: ReactNode | null;
  okText?: ReactNode;
  okType?: string;
  cancelText?: ReactNode;
  onOk?: () => unknown;
  okButtonProps?: { disabled?: boolean; hidden?: boolean; loading?: boolean };
  cancelButtonProps?: { disabled?: boolean; hidden?: boolean };
  confirmLoading?: boolean;
  closable?: boolean;
  maskClosable?: boolean;
  width?: number | string;
  centered?: boolean;
  // Accessible name for the role="dialog" wrapper when the modal has no visible `title` (e.g. the
  // recaptcha modal). Ignored when `title` is set — aria-labelledby points at the header instead.
  ariaLabel?: string;
  className?: string;
  style?: CSSProperties;
  bodyStyle?: CSSProperties;
  zIndex?: number;
  afterClose?: () => void;
  children?: ReactNode;
}

export function DialogContent({
  title,
  footer,
  okText,
  okType = 'primary',
  cancelText,
  onOk,
  okButtonProps,
  cancelButtonProps,
  confirmLoading = false,
  closable = true,
  maskClosable = true,
  width = 520,
  centered = false,
  ariaLabel,
  className,
  style,
  bodyStyle,
  zIndex,
  afterClose,
  children,
}: DialogContentProps) {
  const { open, onOpenChange } = useContext(DialogContext);
  const wrapRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const sentinelStartRef = useRef<HTMLDivElement>(null);
  const sentinelEndRef = useRef<HTMLDivElement>(null);
  const lastFocusRef = useRef<HTMLElement | null>(null);
  const openTime = useRef(0);
  const dialogMouseDown = useRef(false);
  const titleIdRef = useRef<string>('');
  if (!titleIdRef.current) titleIdRef.current = `rcDialogTitle${titleIdSeed++}`;
  const lockedRef = useRef(false);
  const firstOpen = useRef(true);
  const rafRef = useRef<number | undefined>(undefined);
  const prevOpen = useRef(false);

  // destroyOnClose:false — once opened, the dialog stays mounted (typed inputs persist across
  // close/reopen) and is hidden with display:none when fully closed. renderedRef flips true
  // synchronously on the first open so the body (and any autofocus target) is in the DOM
  // immediately. `phase`/`active` reproduce rc-animate's class lifecycle.
  const renderedRef = useRef(open);
  if (open) renderedRef.current = true;
  const [phase, setPhase] = useState<MotionPhase>(null);
  const [active, setActive] = useState(false);

  // X / Esc / mask-click / the default Cancel button all map to rc-dialog's onClose; the
  // call-site's onOpenChange carries any cancel-side cleanup, exactly like antd's onCancel.
  const requestClose = () => onOpenChange(false);

  function doLock() {
    if (!lockedRef.current) {
      lockedRef.current = true;
      lockBodyScroll();
    }
  }
  function doUnlock() {
    if (lockedRef.current) {
      lockedRef.current = false;
      unlockBodyScroll();
    }
  }

  // Drive the open/close lifecycle in a layout effect so the `${name}-${phase}` class lands
  // before paint (no first-frame flash). `${name}-${phase}-active` is then added on the next
  // frame to start the @keyframes; animationend clears the classes. On open we also lock
  // scroll, set the zoom origin from the last click point, and focus the dialog; on close we
  // start the leave animation and restore focus to the trigger (rc-dialog's behaviour).
  useLayoutEffect(() => {
    if (open === prevOpen.current) return;
    prevOpen.current = open;
    if (rafRef.current) cancelAnimationFrame(rafRef.current);
    if (open) {
      setPhase(firstOpen.current ? 'appear' : 'enter');
      firstOpen.current = false;
      openTime.current = Date.now();
      doLock();
      const dialogNode = dialogRef.current;
      if (dialogNode) {
        if (modalMousePosition) {
          const rect = dialogNode.getBoundingClientRect();
          dialogNode.style.transformOrigin = `${modalMousePosition.x - rect.left}px ${modalMousePosition.y - rect.top}px`;
        } else {
          dialogNode.style.transformOrigin = '';
        }
      }
      // rc-dialog's tryFocus: unless focus is already inside the wrap, save the outside node
      // (to restore on close) and focus the start sentinel.
      if (!wrapRef.current?.contains(document.activeElement)) {
        lastFocusRef.current = document.activeElement as HTMLElement | null;
        sentinelStartRef.current?.focus();
      }
    } else {
      setPhase('leave');
      lastFocusRef.current?.focus?.();
      lastFocusRef.current = null;
    }
    setActive(false);
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = requestAnimationFrame(() => setActive(true));
    });
  }, [open]);

  // Release the scroll-lock if the modal unmounts while still open (leave never completes).
  useEffect(() => () => doUnlock(), []);

  const onDialogAnimationEnd = (event: { target: EventTarget }) => {
    if (event.target !== dialogRef.current) return;
    // Clearing the phase while closed flips the wrap to display:none (rc-dialog's onAnimateLeave).
    if (phase === 'leave') {
      doUnlock();
      afterClose?.();
    }
    setPhase(null);
    setActive(false);
  };

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.stopPropagation();
      requestClose();
      return;
    }
    if (event.key === 'Tab') {
      const activeElement = document.activeElement;
      if (event.shiftKey) {
        if (activeElement === sentinelStartRef.current) sentinelEndRef.current?.focus();
      } else if (activeElement === sentinelEndRef.current) {
        sentinelStartRef.current?.focus();
      }
    }
  };

  const onMaskClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (Date.now() - openTime.current < 300) return;
    if (event.target !== event.currentTarget) return;
    if (dialogMouseDown.current) return;
    if (!maskClosable) return;
    requestClose();
  };

  const onMaskMouseUp = () => {
    if (dialogMouseDown.current) {
      setTimeout(() => {
        dialogMouseDown.current = false;
      }, 0);
    }
  };

  if (!renderedRef.current) return null;

  // display:none once fully closed (closed and no longer animating out) — rc-dialog's hidden wrap.
  const visuallyClosed = !open && phase !== 'leave';
  const motionClass = (name: string) =>
    phase ? `${name}-${phase}${active ? ` ${name}-${phase}-active` : ''}` : '';

  const footerContent =
    footer === undefined ? (
      <div>
        <AntBtn
          type="button"
          className="ant-btn"
          hidden={cancelButtonProps?.hidden}
          disabled={cancelButtonProps?.disabled}
          onClick={requestClose}
        >
          {cancelText}
        </AntBtn>
        <AntBtn
          type="button"
          className={cn(
            'ant-btn',
            okType && `ant-btn-${okType}`,
            (confirmLoading || okButtonProps?.loading) && 'ant-btn-loading',
          )}
          hidden={okButtonProps?.hidden}
          disabled={okButtonProps?.disabled}
          onClick={() => onOk?.()}
        >
          {confirmLoading || okButtonProps?.loading ? <LegacyLoadingIcon /> : null}
          {okText}
        </AntBtn>
      </div>
    ) : (
      footer
    );
  const hasFooter = footer === undefined || Boolean(footer);

  return createPortal(
    <div className="ant-modal-root">
      <div
        className={cn('ant-modal-mask', visuallyClosed && 'ant-modal-mask-hidden', motionClass('fade'))}
        style={zIndex ? { zIndex } : undefined}
      />
      <div
        ref={wrapRef}
        tabIndex={-1}
        className={cn('ant-modal-wrap', centered && 'ant-modal-centered')}
        role="dialog"
        aria-labelledby={title ? titleIdRef.current : undefined}
        aria-label={title ? undefined : ariaLabel}
        style={{ ...(zIndex ? { zIndex } : {}), display: visuallyClosed ? 'none' : undefined }}
        onKeyDown={onKeyDown}
        onClick={onMaskClick}
        onMouseUp={onMaskMouseUp}
      >
        <div
          ref={dialogRef}
          role="document"
          className={cn('ant-modal', className, motionClass('zoom'))}
          style={{ width, ...style }}
          onMouseDown={() => {
            dialogMouseDown.current = true;
          }}
          onAnimationEnd={onDialogAnimationEnd}
        >
          <div ref={sentinelStartRef} tabIndex={0} aria-hidden="true" style={SENTINEL_STYLE} />
          <div className="ant-modal-content">
            {closable && (
              <button type="button" aria-label="Close" className="ant-modal-close" onClick={requestClose}>
                <span className="ant-modal-close-x">
                  <CloseIcon className="ant-modal-close-icon" />
                </span>
              </button>
            )}
            {title ? (
              <div className="ant-modal-header">
                <div className="ant-modal-title" id={titleIdRef.current}>
                  {title}
                </div>
              </div>
            ) : null}
            <div className="ant-modal-body" style={bodyStyle}>
              {children}
            </div>
            {hasFooter ? <div className="ant-modal-footer">{footerContent}</div> : null}
          </div>
          <div ref={sentinelEndRef} tabIndex={0} aria-hidden="true" style={SENTINEL_STYLE} />
        </div>
      </div>
    </div>,
    document.body,
  );
}
