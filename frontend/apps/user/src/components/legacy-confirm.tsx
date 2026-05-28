import { useEffect, useState, type ReactNode } from 'react';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { AntBtn } from '@/components/ant-btn';

interface LegacyConfirmOptions {
  title: ReactNode;
  content?: ReactNode;
  okText?: ReactNode;
  cancelText?: ReactNode;
  maskClosable?: boolean;
  showCancel?: boolean;
}

interface LegacyConfirmRequest {
  id: number;
  options: LegacyConfirmOptions;
  resolve: (value: boolean) => void;
}

let nextId = 1;
let queue: LegacyConfirmRequest[] = [];
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((listener) => listener());
}

function currentRequest() {
  return queue[0] ?? null;
}

export function legacyConfirm(options: LegacyConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    queue = [...queue, { id: nextId++, options, resolve }];
    emit();
  });
}

export function LegacyConfirmProvider() {
  const [request, setRequest] = useState<LegacyConfirmRequest | null>(() => currentRequest());

  useEffect(() => {
    const listener = () => setRequest(currentRequest());
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  const close = (value: boolean) => {
    if (!request) return;
    request.resolve(value);
    queue = queue.filter((item) => item.id !== request.id);
    emit();
  };

  const options = request?.options;

  return (
    <Dialog
      open={Boolean(request)}
      onOpenChange={(open) => {
        if (!open && options?.maskClosable) close(false);
      }}
    >
      <DialogContent
        showClose={false}
        className="v2board-ant-confirm-modal ant-modal-confirm ant-modal-confirm-confirm"
        onEscapeKeyDown={(event) => {
          if (options?.maskClosable) close(false);
          else event.preventDefault();
        }}
        onPointerDownOutside={(event) => {
          if (options?.maskClosable) close(false);
          else event.preventDefault();
        }}
      >
        <div className="ant-modal-body">
          <div className="ant-modal-confirm-body-wrapper">
            <div className="ant-modal-confirm-body">
              <i className="anticon anticon-exclamation-circle" />
              <div className="ant-modal-confirm-title">{options?.title}</div>
              {options?.content && (
                <div className="ant-modal-confirm-content">{options.content}</div>
              )}
            </div>
            <div className="ant-modal-confirm-btns">
              {options?.showCancel !== false && (
                <AntBtn type="button" className="ant-btn" onClick={() => close(false)}>
                  {options?.cancelText ?? '取消'}
                </AntBtn>
              )}
              <AntBtn type="button" className="ant-btn ant-btn-primary" onClick={() => close(true)}>
                {options?.okText ?? '确定'}
              </AntBtn>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
