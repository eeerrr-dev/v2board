declare module 'qrcode.react' {
  import type { ComponentType, CSSProperties } from 'react';

  export interface QRCodeProps {
    value: string;
    renderAs?: 'canvas' | 'svg';
    size?: number;
    bgColor?: string;
    fgColor?: string;
    level?: 'L' | 'M' | 'Q' | 'H';
    includeMargin?: boolean;
    style?: CSSProperties;
  }

  const QRCode: ComponentType<QRCodeProps>;
  export default QRCode;
}
