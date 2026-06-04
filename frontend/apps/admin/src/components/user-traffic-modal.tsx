import { useEffect, useRef, useState } from 'react';
import { Modal, Table } from 'antd';
import type { TablePaginationConfig } from 'antd';
import { formatBytes, formatDate } from '@v2board/config/format';
import { useAdminUserTraffic } from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';

interface LegacyTrafficPagination {
  current?: number;
  page?: number;
  pageSize: number;
  total?: number;
}

export function UserTrafficModal({
  userId,
  open,
  onClose,
}: {
  userId?: number | null;
  open: boolean;
  onClose: () => void;
}) {
  const [pagination, setPagination] = useState<LegacyTrafficPagination>({
    page: 1,
    pageSize: 10,
    total: 0,
  });
  const lastUserIdRef = useRef<number | null | undefined>(undefined);
  const records = useAdminUserTraffic(
    userId ?? undefined,
    pagination,
    open,
  );

  useEffect(() => {
    if (!open || userId == null) return;
    if (lastUserIdRef.current !== undefined && lastUserIdRef.current !== userId) {
      setPagination({ page: 1, pageSize: 10, total: 0 });
    }
    lastUserIdRef.current = userId;
  }, [open, userId]);

  return (
    <Modal
      width="100%"
      style={{ maxWidth: 1000, padding: '0 10px', top: 20 }}
      styles={{ body: { padding: 0 } }}
      footer={false}
      open={open}
      title="流量记录"
      onCancel={onClose}
    >
      <LegacySpin loading={records.isFetching}>
        <Table
          dataSource={records.data?.data ?? []}
          pagination={{
            ...pagination,
            total: records.data?.total,
            size: 'small',
          }}
          columns={[
            {
              title: '日期',
              dataIndex: 'record_at',
              key: 'record_at',
              render: (value: number) => formatDate(value),
            },
            {
              title: '上行',
              dataIndex: 'u',
              key: 'd',
              align: 'right',
              render: (value: number) => formatBytes(value),
            },
            {
              title: '下行',
              dataIndex: 'd',
              key: 'd',
              align: 'right',
              render: (value: number) => formatBytes(value),
            },
            {
              title: '倍率',
              dataIndex: 'server_rate',
              key: 'server_rate',
              align: 'right',
            },
          ]}
          onChange={(next: TablePaginationConfig) =>
            setPagination(next as LegacyTrafficPagination)
          }
        />
      </LegacySpin>
    </Modal>
  );
}
