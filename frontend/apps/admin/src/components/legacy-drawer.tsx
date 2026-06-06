import { useEffect, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { LegacyCloseIcon } from './legacy-ant-icon';

interface LegacyDrawerProps {
  children: ReactNode;
  id?: string;
  maskClosable?: boolean;
  open: boolean;
  title: ReactNode;
  width?: number | string;
  onClose: () => void;
}

function drawerWidthStyle(width: number | string | undefined) {
  if (width === undefined) return undefined;
  return { width };
}

export function LegacyDrawer({
  children,
  id,
  maskClosable = true,
  open,
  title,
  width,
  onClose,
}: LegacyDrawerProps) {
  useEffect(() => {
    if (!open || typeof document === 'undefined') return;
    const previousOverflow = document.body.style.overflow;
    const previousTouchAction = document.body.style.touchAction;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };

    document.body.style.overflow = 'hidden';
    document.body.style.touchAction = 'none';
    window.addEventListener('keydown', onKeyDown);

    return () => {
      document.body.style.overflow = previousOverflow;
      document.body.style.touchAction = previousTouchAction;
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [onClose, open]);

  if (!open || typeof document === 'undefined') return null;

  return createPortal(
    <div id={id} tabIndex={-1} className="ant-drawer ant-drawer-right ant-drawer-open">
      <div className="ant-drawer-mask" onClick={maskClosable ? onClose : undefined} />
      <div className="ant-drawer-content-wrapper" style={drawerWidthStyle(width)}>
        <div className="ant-drawer-content">
          <div className="ant-drawer-wrapper-body">
            <div className="ant-drawer-header">
              <div className="ant-drawer-title">{title}</div>
              <button aria-label="Close" className="ant-drawer-close" onClick={onClose}>
                <LegacyCloseIcon />
              </button>
            </div>
            <div className="ant-drawer-body">{children}</div>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}
