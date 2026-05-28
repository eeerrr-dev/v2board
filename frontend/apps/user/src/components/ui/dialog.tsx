import * as DialogPrimitive from '@radix-ui/react-dialog';
import {
  forwardRef,
  type ComponentPropsWithoutRef,
  type ElementRef,
} from 'react';
import { cn } from '@/lib/cn';

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogPortal = DialogPrimitive.Portal;
export const DialogClose = DialogPrimitive.Close;

export const DialogOverlay = forwardRef<
  ElementRef<typeof DialogPrimitive.Overlay>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Overlay
    ref={ref}
    className={cn('v2board-dialog-overlay', className)}
    {...props}
  />
));
DialogOverlay.displayName = 'DialogOverlay';

interface DialogContentProps
  extends ComponentPropsWithoutRef<typeof DialogPrimitive.Content> {
  showClose?: boolean;
  centered?: boolean;
  zIndex?: number;
}

export const DialogContent = forwardRef<
  ElementRef<typeof DialogPrimitive.Content>,
  DialogContentProps
>(({ className, children, showClose = true, centered = false, zIndex, ...props }, ref) => (
  <DialogPortal>
    <DialogOverlay className="ant-modal-mask" style={zIndex ? { zIndex } : undefined} />
    <div className="ant-modal-wrap v2board-dialog-wrap" style={zIndex ? { zIndex } : undefined}>
      <DialogPrimitive.Content
        ref={ref}
        className={cn(
          'v2board-dialog-content ant-modal',
          centered && 'v2board-dialog-centered',
          className,
        )}
        {...props}
      >
        <div className="ant-modal-content">
          {showClose && (
            <DialogPrimitive.Close className="ant-modal-close v2board-dialog-close">
              <span className="ant-modal-close-x">
                <i className="anticon anticon-close" aria-hidden />
              </span>
              <span className="sr-only">Close</span>
            </DialogPrimitive.Close>
          )}
          {children}
        </div>
      </DialogPrimitive.Content>
    </div>
  </DialogPortal>
));
DialogContent.displayName = 'DialogContent';
