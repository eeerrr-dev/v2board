import { useEffect, useRef, useState } from 'react';
import { Modal } from 'antd';
import type { admin } from '@v2board/api-client';
import { formatBytes, formatDate } from '@v2board/config/format';
import { useAdminUserTraffic } from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';
import {
  LegacyStandaloneTable,
  LegacyTablePagination,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
  type LegacyTablePaginationChange,
} from '@/components/legacy-standalone-table';

interface LegacyTrafficPagination {
  current?: number;
  page?: number;
  pageSize: number;
  total?: number;
}

const headers: LegacyStandaloneTableHeader[] = [
  { title: '日期' },
  { title: '上行', alignRight: true },
  { title: '下行', alignRight: true },
  { title: '倍率', alignRight: true },
];

function getTrafficCurrentPage(pagination: LegacyTrafficPagination) {
  return pagination.current ?? pagination.page ?? 1;
}

function renderTrafficRow(record: admin.AdminUserTrafficRecord, index: number) {
  return (
    <tr key={index} className="ant-table-row ant-table-row-level-0" {...legacyTableRowKey(index)}>
      <td>{formatDate(record.record_at)}</td>
      <td style={{ textAlign: 'right' }}>{formatBytes(record.u)}</td>
      <td style={{ textAlign: 'right' }}>{formatBytes(record.d)}</td>
      <td style={{ textAlign: 'right' }}>{record.server_rate}</td>
    </tr>
  );
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
  const records = useAdminUserTraffic(userId ?? undefined, pagination, open);

  useEffect(() => {
    if (!open || userId == null) return;
    if (lastUserIdRef.current !== undefined && lastUserIdRef.current !== userId) {
      setPagination({ page: 1, pageSize: 10, total: 0 });
    }
    lastUserIdRef.current = userId;
  }, [open, userId]);

  const data = records.data?.data ?? [];
  const total = records.data?.total ?? pagination.total;
  const current = getTrafficCurrentPage(pagination);
  const nextPagination = { ...pagination, current, total };
  const handlePaginationChange = (next: LegacyTablePaginationChange) => {
    setPagination({ ...next, total });
  };

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
        <LegacyStandaloneTable
          headers={headers}
          isEmpty={data.length === 0}
          pagination={
            <LegacyTablePagination
              current={current}
              pageSize={nextPagination.pageSize}
              total={nextPagination.total}
              onChange={handlePaginationChange}
            />
          }
        >
          {data.map(renderTrafficRow)}
        </LegacyStandaloneTable>
      </LegacySpin>
    </Modal>
  );
}
