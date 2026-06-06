import { Fragment, cloneElement, useState, type ReactElement, type ReactNode } from 'react';
import { App } from 'antd';
import type { AdminFilter } from '@v2board/api-client';
import { LegacyButton } from './legacy-button';
import { LegacyDatePicker } from './legacy-date-picker';
import { LegacyDeleteIcon, LegacyPlusIcon } from './legacy-ant-icon';
import { LegacyDrawer } from './legacy-drawer';
import { LegacyInput } from './legacy-input';
import { LegacySelect, type LegacySelectValue } from './legacy-select';

export interface LegacyFilterOption {
  key?: ReactNode;
  label?: ReactNode;
  value: string | number;
}

export interface LegacyFilterKey {
  key: string;
  title: string;
  condition: string[];
  type?: 'select' | 'date';
  options?: LegacyFilterOption[];
}

function defaultFilter(keys: LegacyFilterKey[]): AdminFilter {
  const first = keys[0]!;
  return {
    key: first.key,
    condition: first.condition[0]!,
    value: '',
  };
}

function isBlank(value: AdminFilter['value']) {
  return value === '';
}

function LegacyDivider({ children }: { children: ReactNode }) {
  return (
    <div
      className="ant-divider ant-divider-horizontal ant-divider-with-text-center"
      role="separator"
    >
      <span className="ant-divider-inner-text">{children}</span>
    </div>
  );
}

export function LegacyFilterDrawer({
  value,
  keys,
  onChange,
  children,
}: {
  value: AdminFilter[];
  keys: LegacyFilterKey[];
  onChange: (value: AdminFilter[]) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const { notification } = App.useApp();
  const [open, setOpen] = useState(false);
  const [filters, setFilters] = useState<AdminFilter[]>(value || []);
  const [keyIndex, setKeyIndex] = useState(0);

  const add = () => setFilters((state) => [...state, defaultFilter(keys)]);
  const hide = () => setOpen(false);
  const remove = (index: number) =>
    setFilters((state) => state.filter((_, currentIndex) => currentIndex !== index));
  const update = (index: number, patch: Partial<AdminFilter>) =>
    setFilters((state) =>
      state.map((item, currentIndex) => {
        if (currentIndex !== index) return item;
        if (patch.key) {
          const nextIndex = keys.findIndex((key) => key.key === patch.key);
          const next = keys[nextIndex]!;
          setKeyIndex(nextIndex);
          return {
            ...item,
            key: next.key,
            condition: next.condition[0]!,
          };
        }
        return { ...item, ...patch };
      }),
    );

  const reset = () => {
    setFilters([]);
    onChange([]);
    hide();
  };

  const search = () => {
    let valid = true;
    filters.forEach((filter) => {
      if (isBlank(filter.value)) {
        notification.error({
          message: '过滤器',
          description: '欲检索内容不能为空',
          duration: 1.5,
        });
        valid = false;
      }
    });
    if (!valid) return;
    onChange(filters);
    hide();
  };

  return (
    <>
      {cloneElement(children, { onClick: () => setOpen(true) })}
      <LegacyDrawer
        title="过滤器"
        open={open}
        onClose={hide}
        className="v2board-filter-drawer"
        footer={<></>}
        width={256}
      >
        {filters.map((filter, index) => {
          const selected = keys.find((key) => key.key === filter.key)!;
          return (
            <Fragment key={index}>
              <LegacyDivider>
                条件{index + 1}{' '}
                <LegacyDeleteIcon
                  tabIndex={-1}
                  style={{ color: '#ff4d4f' }}
                  onClick={() => remove(index)}
                />
              </LegacyDivider>
              <div className="form-group">
                <label>字段名</label>
                <div>
                  <LegacySelect
                    value={filter.key}
                    style={{ width: '100%' }}
                    options={keys.map((item) => ({ value: item.key, label: item.title }))}
                    onChange={(key) => update(index, { key: key as string })}
                  />
                </div>
              </div>
              <div className="form-group">
                <label>条件</label>
                <div>
                  <LegacySelect
                    value={filter.condition}
                    style={{ width: '100%' }}
                    options={keys[keyIndex]!.condition.map((condition) => ({
                      value: condition,
                      label: condition,
                    }))}
                    onChange={(condition) => update(index, { condition: condition as string })}
                  />
                </div>
              </div>
              <div className="form-group">
                <label>欲检索内容</label>
                <div>
                  {selected.type === 'select' ? (
                    <LegacySelect
                      value={(filter.value || undefined) as LegacySelectValue | undefined}
                      style={{ width: '100%' }}
                      placeholder="请选择值"
                      options={selected.options!.map((option) => ({
                        value: option.value,
                        label: String(option.key ?? option.label ?? option.value),
                      }))}
                      onChange={(filterValue) => update(index, { value: filterValue })}
                    />
                  ) : null}
                  {selected.type === 'date' ? (
                    <LegacyDatePicker
                      style={{ width: '100%' }}
                      onChange={(value) => update(index, { value })}
                    />
                  ) : null}
                  {!selected.type ? (
                    <LegacyInput
                      style={{ width: '100%' }}
                      className="ant-input"
                      defaultValue={filter.value || undefined}
                      placeholder="值"
                      onChange={(event) => update(index, { value: event.target.value })}
                    />
                  ) : null}
                </div>
              </div>
            </Fragment>
          );
        })}
        <LegacyButton className="ant-btn ant-btn-primary" style={{ width: '100%' }} onClick={add}>
          <LegacyPlusIcon />
          <span> 添加条件</span>
        </LegacyButton>
        <div className="v2board-drawer-action">
          <LegacyButton
            disabled={!filters.length}
            className="ant-btn ant-btn-danger"
            onClick={reset}
            style={{ float: 'left' }}
          >
            重置
          </LegacyButton>
          <LegacyButton className="ant-btn" style={{ marginRight: 8 }} onClick={hide}>
            取消
          </LegacyButton>
          <LegacyButton
            disabled={!filters.length}
            className="ant-btn ant-btn-primary"
            onClick={search}
          >
            检索
          </LegacyButton>
        </div>
      </LegacyDrawer>
    </>
  );
}
