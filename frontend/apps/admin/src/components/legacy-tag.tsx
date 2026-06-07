import type { CSSProperties, MouseEventHandler, ReactNode } from 'react';

interface LegacyTagProps {
  children?: ReactNode;
  style?: CSSProperties;
  onClick?: MouseEventHandler<HTMLSpanElement>;
}

export function LegacyTag({ children, style, onClick }: LegacyTagProps) {
  return (
    <span className="ant-tag" style={style} onClick={onClick}>
      {children}
    </span>
  );
}
