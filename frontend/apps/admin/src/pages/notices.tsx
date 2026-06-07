import { useEffect, useState } from 'react';
import dayjs from 'dayjs';
import type { Notice } from '@v2board/types';
import {
  useAdminNotices,
  useDropNoticeMutation,
  useSaveNoticeMutation,
  useShowNoticeMutation,
} from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';
import { LegacyButton } from '@/components/legacy-button';
import { LegacyModal } from '@/components/legacy-modal';
import { LegacyLoadingIcon, LegacyPlusIcon } from '@/components/legacy-ant-icon';
import {
  LegacyStandaloneTable,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
} from '@/components/legacy-standalone-table';
import { LegacySwitch } from '@/components/legacy-switch';
import { LegacySelect } from '@/components/legacy-select';
import { LegacyInput, LegacyTextArea } from '@/components/legacy-input';

export default function NoticesPage() {
  const notices = useAdminNotices({});
  const save = useSaveNoticeMutation();
  const drop = useDropNoticeMutation();
  const show = useShowNoticeMutation();
  const [visible, setVisible] = useState(false);
  const [submit, setSubmit] = useState<Partial<Notice>>({});
  const [saveLoading] = useState<boolean | undefined>(undefined);
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

  const headers: LegacyStandaloneTableHeader[] = [
    { title: '#' },
    { title: '显示' },
    { title: '标题' },
    { title: '创建时间', alignRight: true },
    { title: '操作', alignRight: true, fixedRight: true },
  ];

  const renderNoticeShowSwitch = (value: 0 | 1, row: Notice) => (
    <LegacySwitch
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
  );

  const renderNoticeActions = (row: Notice, index: number) => (
    <div>
      <a
        onClick={() => {
          setSubmit(dataSource[index] as Partial<Notice>);
          setVisible(true);
        }}
        ref={legacyHref()}
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
        ref={legacyHref()}
      >
        删除
      </a>
    </div>
  );

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={notices.isFetching}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <LegacyButton className="ant-btn" onClick={modalVisible}>
                <LegacyPlusIcon />
                <span> 添加公告</span>
              </LegacyButton>
            </div>
            <LegacyStandaloneTable
              headers={headers}
              isEmpty={dataSource.length === 0}
              scrollX={950}
              fixedRightChildren={dataSource.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyTableRowKey(index)}
                >
                  <td className="ant-table-row-cell-last" style={{ textAlign: 'right' }}>
                    {renderNoticeActions(row, index)}
                  </td>
                </tr>
              ))}
            >
              {dataSource.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyTableRowKey(index)}
                >
                  <td className="">{row.id}</td>
                  <td className="">{renderNoticeShowSwitch(row.show, row)}</td>
                  <td className="">{row.title}</td>
                  <td className="" style={{ textAlign: 'right' }}>
                    {dayjs(1000 * row.created_at).format('YYYY/MM/DD HH:mm')}
                  </td>
                  <td
                    className="ant-table-fixed-columns-in-body ant-table-row-cell-last"
                    style={{ textAlign: 'right' }}
                  >
                    {renderNoticeActions(row, index)}
                  </td>
                </tr>
              ))}
            </LegacyStandaloneTable>
          </div>
        </div>
      </LegacySpin>
      <LegacyModal
        title={`${submit.id ? '编辑公告' : '新建公告'}`}
        visible={visible}
        onCancel={modalVisible}
        onOk={() => {
          saveLoading || void saveNotice();
        }}
        okText={saveLoading ? <LegacyLoadingIcon /> : '提交'}
        cancelText="取消"
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">标题</label>
            <LegacyInput
              className="ant-input"
              placeholder="请输入公告标题"
              value={submit.title}
              onChange={(event) => setSubmit({ ...submit, title: event.target.value })}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">公告内容</label>
            <LegacyTextArea
              className="ant-input"
              rows={12}
              value={submit.content}
              placeholder="请输入公告内容"
              onChange={(event) => setSubmit({ ...submit, content: event.target.value })}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">公告标签</label>
            <LegacySelect
              mode="tags"
              value={submit.tags || []}
              style={{ width: '100%' }}
              placeholder="输入后回车添加标签"
              options={[]}
              onChange={(tags) => {
                setSubmit({ ...submit, tags: tags.length > 0 ? tags.map(String) : null });
              }}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">图片URL</label>
            <LegacyInput
              className="ant-input"
              placeholder="请输入图片URL"
              value={submit.img_url as string | undefined}
              onChange={(event) => setSubmit({ ...submit, img_url: event.target.value })}
            />
          </div>
        </div>
      </LegacyModal>
    </>
  );
}
