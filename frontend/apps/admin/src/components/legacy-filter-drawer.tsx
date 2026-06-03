import { cloneElement, useState, type ReactElement, type ReactNode } from 'react';
import { App, Button, DatePicker, Divider, Drawer, Input, Select } from 'antd';
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import dayjs from 'dayjs';
import type { AdminFilter } from '@v2board/api-client';

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
    if (filters.some((filter) => isBlank(filter.value))) {
      notification.error({
        message: '过滤器',
        description: '欲检索内容不能为空',
        duration: 1.5,
      });
      return;
    }
    onChange(filters);
    hide();
  };

  return (
    <>
      {cloneElement(children, { onClick: () => setOpen(true) })}
      <Drawer
        title="过滤器"
        open={open}
        onClose={hide}
        className="v2board-filter-drawer"
        footer={<></>}
      >
        {filters.map((filter, index) => {
          const selected = keys.find((key) => key.key === filter.key)!;
          return (
            <>
              <Divider>
                条件{index + 1}{' '}
                <DeleteOutlined style={{ color: '#ff4d4f' }} onClick={() => remove(index)} />
              </Divider>
              <div className="form-group">
                <label>字段名</label>
                <div>
                  <Select
                    value={filter.key}
                    style={{ width: '100%' }}
                  >
                    {keys.map((item, optionIndex) => (
                      <Select.Option
                        key={optionIndex}
                        value={item.key}
                        onClick={() => update(index, { key: item.key })}
                      >
                        {item.title}
                      </Select.Option>
                    ))}
                  </Select>
                </div>
              </div>
              <div className="form-group">
                <label>条件</label>
                <div>
                  <Select
                    value={filter.condition}
                    style={{ width: '100%' }}
                    onChange={(condition) => update(index, { condition })}
                  >
                    {keys[keyIndex]!.condition.map((condition) => (
                      <Select.Option key={condition} value={condition}>
                        {condition}
                      </Select.Option>
                    ))}
                  </Select>
                </div>
              </div>
              <div className="form-group">
                <label>欲检索内容</label>
                <div>
                  {selected.type === 'select' ? (
                    <Select
                      defaultValue={filter.value || undefined}
                      style={{ width: '100%' }}
                      placeholder="请选择值"
                      onChange={(filterValue) => update(index, { value: filterValue })}
                    >
                      {selected.options!.map((option) => (
                        <Select.Option value={option.value}>{option.key}</Select.Option>
                      ))}
                    </Select>
                  ) : null}
                  {selected.type === 'date' ? (
                    <DatePicker
                      style={{ width: '100%' }}
                      showTime={{ defaultValue: dayjs().startOf('day') }}
                      onChange={(date) => update(index, { value: date && date.format('X') })}
                    />
                  ) : null}
                  {!selected.type ? (
                    <Input
                      style={{ width: '100%' }}
                      defaultValue={filter.value || undefined}
                      placeholder="值"
                      onChange={(event) => update(index, { value: event.target.value })}
                    />
                  ) : null}
                </div>
              </div>
            </>
          );
        })}
        <Button style={{ width: '100%' }} type="primary" onClick={add}>
          <PlusOutlined /> 添加条件
        </Button>
        <div className="v2board-drawer-action">
          <Button disabled={!filters.length} danger onClick={reset} style={{ float: 'left' }}>
            重置
          </Button>
          <Button style={{ marginRight: 8 }} onClick={hide}>
            取消
          </Button>
          <Button disabled={!filters.length} onClick={search} type="primary">
            检索
          </Button>
        </div>
      </Drawer>
    </>
  );
}
