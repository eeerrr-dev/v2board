import { useEffect, type CSSProperties, type MouseEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { LegacyCloseIcon, LegacyLoadingIcon } from './legacy-ant-icon';
import { LegacyButton } from './legacy-button';

interface LegacyModalProps {
  bodyStyle?: CSSProperties;
  children: ReactNode;
  cancelText?: ReactNode;
  footer?: ReactNode | boolean | null;
  maskClosable?: boolean;
  okButtonProps?: { loading?: boolean };
  okText?: ReactNode;
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

export function LegacyModal({
  bodyStyle,
  children,
  cancelText = '取消',
  footer,
  maskClosable = true,
  okButtonProps,
  okText = '确定',
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
                      {cancelText}
                    </LegacyButton>
                    <LegacyButton
                      className={`ant-btn ant-btn-primary${okLoading ? ' ant-btn-loading' : ''}`}
                      onClick={onOk}
                    >
                      {okLoading ? <LegacyLoadingIcon /> : null}
                      {okText}
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
