import type { ReactNode } from 'react';

export function LegacySwitch({
  checked,
  checkedChildren,
  onChange,
  size,
  unCheckedChildren,
}: {
  checked: boolean | number | string;
  checkedChildren?: ReactNode;
  onChange?: (checked: boolean) => void;
  size?: 'small';
  unCheckedChildren?: ReactNode;
}) {
  const enabled = Boolean(checked);

  return (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      className={`${size === 'small' ? 'ant-switch-small ' : ''}ant-switch${
        enabled ? ' ant-switch-checked' : ''
      }`}
      onClick={() => onChange?.(!enabled)}
    >
      <span className="ant-switch-inner">{enabled ? checkedChildren : unCheckedChildren}</span>
    </button>
  );
}
