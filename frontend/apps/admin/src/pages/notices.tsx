import { useEffect, useState } from 'react';
import { Button, Input, Modal, Select, Switch, Table } from 'antd';
import type { TableProps } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import dayjs from 'dayjs';
import type { Notice } from '@v2board/types';
import {
  useAdminNotices,
  useDropNoticeMutation,
  useSaveNoticeMutation,
  useShowNoticeMutation,
} from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';

export default function NoticesPage() {
  const notices = useAdminNotices({});
  const save = useSaveNoticeMutation();
  const drop = useDropNoticeMutation();
  const show = useShowNoticeMutation();
  const [visible, setVisible] = useState(false);
  const [submit, setSubmit] = useState<Partial<Notice>>({});
  const dataSource = notices.data?.data ?? [];

  useEffect(() => {
    if (!visible) setSubmit({});
  }, [visible]);

  const modalVisible = () => {
    setVisible((current) => !current);
  };

  const saveNotice = async () => {
    await save.mutateAsync({ ...submit });
    void notices.refetch();
    modalVisible();
  };

  const columns: TableProps<Notice>['columns'] = [
    {
      title: '#',
      dataIndex: 'id',
      key: 'id',
    },
    {
      title: '显示',
      dataIndex: 'show',
      key: 'show',
      render: (value: 0 | 1, row) => (
        <Switch
          size="small"
          onChange={() =>
            show.mutate(row.id, {
              onSuccess: () => {
                void notices.refetch();
              },
            })
          }
          checked={value as unknown as boolean}
        />
      ),
    },
    {
      title: '标题',
      dataIndex: 'title',
      key: 'title',
    },
    {
      title: '创建时间',
      dataIndex: 'created_at',
      key: 'created_at',
      align: 'right',
      render: (value: number) => dayjs(1000 * value).format('YYYY/MM/DD HH:mm'),
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      align: 'right',
      fixed: 'right',
      render: (_value, row, index) => (
        <div>
          <a
            onClick={() => {
              setSubmit(dataSource[index] as Partial<Notice>);
              setVisible(true);
            }}
            href="javascript:void(0);"
          >
            编辑
          </a>
          <div className="ant-divider ant-divider-vertical" />
          <a
            onClick={() =>
              drop.mutate(row.id, {
                onSuccess: () => {
                  void notices.refetch();
                },
              })
            }
            href="javascript:void(0);"
          >
            删除
          </a>
        </div>
      ),
    },
  ];

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={notices.isFetching}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <Button onClick={modalVisible}>
                <PlusOutlined /> 添加公告
              </Button>
            </div>
            <Table<Notice>
              tableLayout="auto"
              dataSource={dataSource}
              pagination={false}
              columns={columns}
              scroll={{ x: 950 }}
            />
          </div>
        </div>
      </LegacySpin>
      <Modal
        title={`${submit.id ? '编辑公告' : '新建公告'}`}
        open={visible}
        onCancel={modalVisible}
        onOk={() => void saveNotice()}
        okText="提交"
        cancelText="取消"
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">标题</label>
            <Input
              placeholder="请输入公告标题"
              value={submit.title}
              onChange={(event) => setSubmit({ ...submit, title: event.target.value })}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">公告内容</label>
            <Input.TextArea
              rows={12}
              value={submit.content}
              placeholder="请输入公告内容"
              onChange={(event) => setSubmit({ ...submit, content: event.target.value })}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">公告标签</label>
            <Select
              mode="tags"
              value={submit.tags || []}
              style={{ width: '100%' }}
              placeholder="输入后回车添加标签"
              onChange={(tags) => {
                setSubmit({ ...submit, tags: tags.length > 0 ? tags : null });
              }}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">图片URL</label>
            <Input
              placeholder="请输入图片URL"
              value={submit.img_url as string | undefined}
              onChange={(event) => setSubmit({ ...submit, img_url: event.target.value })}
            />
          </div>
        </div>
      </Modal>
    </>
  );
}
