import { useEffect, type CSSProperties, type MouseEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { LegacyCloseIcon } from './legacy-ant-icon';
import { LegacyButton } from './legacy-button';

interface LegacyModalProps {
  bodyStyle?: CSSProperties;
  children: ReactNode;
  footer?: ReactNode | boolean | null;
  maskClosable?: boolean;
  okButtonProps?: { loading?: boolean };
  open?: boolean;
  style?: CSSProperties;
  styles?: { body?: CSSProperties };
  title: ReactNode;
  visible?: boolean;
  width?: number | string;
  onCancel: () => void;
  onOk?: () => void | Promise<void>;
}

function widthStyle(width: number | string | undefined): CSSProperties {
  const value = width ?? 520;
  return { width: typeof value === 'number' ? `${value}px` : value };
}

function modalStyle(width: number | string | undefined, style: CSSProperties | undefined) {
  return { ...widthStyle(width), ...style };
}

function LegacyModalLoadingIcon() {
  return (
    <i aria-label="图标: loading" className="anticon anticon-loading">
      <svg
        className="anticon-spin"
        viewBox="0 0 1024 1024"
        focusable="false"
        data-icon="loading"
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 0 0-94.3-139.9 437.71 437.71 0 0 0-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z" />
      </svg>
    </i>
  );
}

export function LegacyModal({
  bodyStyle,
  children,
  footer,
  maskClosable = true,
  okButtonProps,
  open,
  style,
  styles,
  title,
  visible,
  width,
  onCancel,
  onOk,
}: LegacyModalProps) {
  const isVisible = visible ?? open ?? false;
  const okLoading = Boolean(okButtonProps?.loading);
  const mergedBodyStyle = bodyStyle ?? styles?.body;

  useEffect(() => {
    if (!isVisible || typeof document === 'undefined') return;

    const hadOpenClass = document.body.classList.contains('ant-modal-open');
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onCancel();
    };

    document.body.classList.add('ant-modal-open');
    window.addEventListener('keydown', onKeyDown);

    return () => {
      if (!hadOpenClass) document.body.classList.remove('ant-modal-open');
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [isVisible, onCancel]);

  if (!isVisible || typeof document === 'undefined') return null;

  const handleMaskClick = (event: MouseEvent<HTMLDivElement>) => {
    if (event.target === event.currentTarget && maskClosable) onCancel();
  };

  return createPortal(
    <div className="ant-modal-root">
      <div className="ant-modal-mask" />
      <div tabIndex={-1} className="ant-modal-wrap" role="dialog" onClick={handleMaskClick}>
        <div className="ant-modal" role="document" style={modalStyle(width, style)}>
          <div className="ant-modal-content">
            <button type="button" aria-label="Close" className="ant-modal-close" onClick={onCancel}>
              <span className="ant-modal-close-x">
                <LegacyCloseIcon />
              </span>
            </button>
            <div className="ant-modal-header">
              <div className="ant-modal-title">{title}</div>
            </div>
            <div className="ant-modal-body" style={mergedBodyStyle}>
              {children}
            </div>
            {footer === false || footer === null ? null : (
              <div className="ant-modal-footer">
                {footer ?? (
                  <>
                    <LegacyButton className="ant-btn" onClick={onCancel}>
                      取消
                    </LegacyButton>
                    <LegacyButton
                      className={`ant-btn ant-btn-primary${okLoading ? ' ant-btn-loading' : ''}`}
                      onClick={onOk}
                    >
                      {okLoading ? <LegacyModalLoadingIcon /> : null}
                      确定
                    </LegacyButton>
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}
