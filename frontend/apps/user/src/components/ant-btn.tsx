import { Children, isValidElement, type ButtonHTMLAttributes, type ReactNode } from 'react';

const TWO_CN_CHAR = /^[一-龥]{2}$/;

// Mirrors Ant Design's autoInsertSpaceInButton: string children are wrapped in a
// <span>, and a single all-Chinese two-character label gets a space inserted
// between the two characters (e.g. "充值" → "充 值").
function insertSpace(children: ReactNode): ReactNode {
  const array = Children.toArray(children);
  const needSpace = array.length === 1 && !array.some((child) => isValidElement(child));
  return Children.map(children, (child) => {
    if (typeof child === 'string') {
      const text = needSpace && TWO_CN_CHAR.test(child) ? child.split('').join(' ') : child;
      return <span>{text}</span>;
    }
    return child;
  });
}

export function AntBtn({ children, ...rest }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button {...rest}>{insertSpace(children)}</button>;
}
