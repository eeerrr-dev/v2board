import {
  cloneElement,
  useEffect,
  useMemo,
  useRef,
  useState,
  type HTMLAttributes,
  type ReactElement,
} from 'react';
import { App, Button, Drawer, Input, Modal, Select, Switch, Table } from 'antd';
import type { TableProps } from 'antd';
import { LoadingOutlined, MenuOutlined, PlusOutlined } from '@ant-design/icons';
import dayjs from 'dayjs';
import MarkdownIt from 'markdown-it';
import { admin } from '@v2board/api-client';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import { apiClient } from '@/lib/api';
import {
  useAdminKnowledge,
  useAdminKnowledgeCategories,
  useDropKnowledgeMutation,
  useSaveKnowledgeMutation,
  useShowKnowledgeMutation,
  useSortKnowledgeMutation,
} from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';

type SaveKnowledgePayload = Parameters<typeof admin.saveKnowledge>[1];

const legacyAdminMarkdown = new MarkdownIt({ html: true, linkify: true, typographer: true });
const LEGACY_KNOWLEDGE_I18N_TEXT = {
  'zh-CN': '简体中文',
  'zh-TW': '繁體中文',
  'en-US': 'English',
  'ja-JP': '日本語',
  'vi-VN': 'Tiếng Việt',
  'ko-KR': '한국어',
} as const;
type LegacyKnowledgeLocale = keyof typeof LEGACY_KNOWLEDGE_I18N_TEXT;
const LEGACY_KNOWLEDGE_LOCALES = (
  Object.keys(LEGACY_KNOWLEDGE_I18N_TEXT) as LegacyKnowledgeLocale[]
).sort();

function renderLegacyAdminMarkdown(markdown: string) {
  return legacyAdminMarkdown.render(markdown);
}

function LegacyMarkdownEditor({
  value,
  onChange,
}: {
  value?: string;
  onChange: (value: string) => void;
}) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [view, setView] = useState({ md: true, html: true });
  const [fullScreen, setFullScreen] = useState(false);
  const text = value ?? '';
  const html = useMemo(() => renderLegacyAdminMarkdown(text), [text]);

  const nextViewInfo = () => {
    if (view.md && view.html) {
      return { view: { md: true, html: false }, icon: 'keyboard', title: '仅显示编辑器' };
    }
    if (view.md) {
      return { view: { md: false, html: true }, icon: 'visibility', title: '仅显示预览' };
    }
    return { view: { md: true, html: true }, icon: 'view-split', title: '显示编辑器与预览' };
  };

  const replaceSelection = (before: string, after = before, placeholder = '') => {
    const textarea = textareaRef.current;
    if (!textarea) {
      onChange(`${text}${before}${placeholder}${after}`);
      return;
    }

    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const selected = text.slice(start, end) || placeholder;
    onChange(`${text.slice(0, start)}${before}${selected}${after}${text.slice(end)}`);
  };

  const insertLine = (prefix: string, placeholder: string) => {
    const textarea = textareaRef.current;
    const insert = `${prefix}${placeholder}`;
    if (!textarea) {
      onChange(`${text}${text ? '\n' : ''}${insert}`);
      return;
    }

    const start = textarea.selectionStart;
    const lineStart = text.lastIndexOf('\n', Math.max(0, start - 1)) + 1;
    onChange(`${text.slice(0, lineStart)}${insert}${text.slice(lineStart)}`);
  };

  const mode = nextViewInfo();

  return (
    <div className={`rc-md-editor ${fullScreen ? 'full' : ''}`} style={{ height: 500 }}>
      <div className="rc-md-navigation visible">
        <div className="navigation-nav left">
          <div className="button-wrap">
            <span
              className="button button-type-header"
              title="标题"
              onClick={() => insertLine('# ', '标题')}
            >
              <i className="rmel-iconfont rmel-icon-font" />
            </span>
            <span
              className="button button-type-bold"
              title="加粗"
              onClick={() => replaceSelection('**', '**', '加粗')}
            >
              <i className="rmel-iconfont rmel-icon-bold" />
            </span>
            <span
              className="button button-type-italic"
              title="斜体"
              onClick={() => replaceSelection('*', '*', '斜体')}
            >
              <i className="rmel-iconfont rmel-icon-italic" />
            </span>
            <span
              className="button button-type-unordered"
              title="无序列表"
              onClick={() => insertLine('- ', '列表')}
            >
              <i className="rmel-iconfont rmel-icon-list-unordered" />
            </span>
            <span
              className="button button-type-quote"
              title="引用"
              onClick={() => insertLine('> ', '引用')}
            >
              <i className="rmel-iconfont rmel-icon-quote" />
            </span>
            <span
              className="button button-type-code"
              title="代码块"
              onClick={() => replaceSelection('```\n', '\n```', 'code')}
            >
              <i className="rmel-iconfont rmel-icon-code" />
            </span>
            <span
              className="button button-type-link"
              title="链接"
              onClick={() => replaceSelection('[', '](https://)', '链接')}
            >
              <i className="rmel-iconfont rmel-icon-link" />
            </span>
          </div>
        </div>
        <div className="navigation-nav right">
          <div className="button-wrap">
            <span
              className="button button-type-mode"
              title={mode.title}
              onClick={() => setView(mode.view)}
            >
              <i className={`rmel-iconfont rmel-icon-${mode.icon}`} />
            </span>
            <span
              className="button button-type-fullscreen"
              title={fullScreen ? '退出全屏' : '全屏'}
              onClick={() => setFullScreen((current) => !current)}
            >
              <i
                className={`rmel-iconfont rmel-icon-${
                  fullScreen ? 'fullscreen-exit' : 'fullscreen'
                }`}
              />
            </span>
          </div>
        </div>
      </div>
      <div className="editor-container">
        <div className="tool-bar">
          <span className="button button-type-menu" title="hidden menu">
            <i className="rmel-iconfont rmel-icon-expand-less" />
          </span>
        </div>
        <section className={`section sec-md ${view.md ? 'visible' : 'in-visible'}`}>
          <textarea
            ref={textareaRef}
            name="textarea"
            value={text}
            className="section-container input"
            wrap="hard"
            onChange={(event) => onChange(event.target.value)}
          />
        </section>
        <section className={`section sec-html ${view.html ? 'visible' : 'in-visible'}`}>
          <div className="section-container html-wrap">
            <div className="custom-html-style" dangerouslySetInnerHTML={{ __html: html }} />
          </div>
        </section>
      </div>
    </div>
  );
}

function KnowledgeEditor({
  id,
  children,
  onSave,
  onSaved,
}: {
  id?: number;
  children: ReactElement<{ onClick?: () => void }>;
  onSave: (payload: SaveKnowledgePayload) => Promise<unknown>;
  onSaved: () => void;
}) {
  const { message } = App.useApp();
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saveLoading, setSaveLoading] = useState(false);
  const [knowledge, setKnowledge] = useState<Partial<Knowledge>>({});
  const [editorKey, setEditorKey] = useState(Math.random());

  const show = async () => {
    setVisible(true);
    setEditorKey(Math.random());
    if (!id) {
      setKnowledge({});
      return;
    }

    setLoading(true);
    try {
      setKnowledge(await admin.knowledgeDetail(apiClient, id));
    } finally {
      setLoading(false);
    }
  };

  const hide = () => {
    setVisible(false);
    setKnowledge({});
  };

  const formChange = (key: keyof Knowledge, value: unknown) => {
    setKnowledge((current) => ({ ...current, [key]: value }));
  };

  const save = async () => {
    setSaveLoading(true);
    try {
      await onSave({ ...knowledge });
    } finally {
      setSaveLoading(false);
    }
    onSaved();
    message.success('保存成功');
  };

  return (
    <>
      {cloneElement(children, { onClick: show })}
      <Drawer
        width="80%"
        id="knowledge"
        open={visible}
        title={id ? '编辑知识' : '新增知识'}
        onClose={hide}
      >
        {loading ? (
          <LoadingOutlined />
        ) : (
          <div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">标题</label>
              <Input
                placeholder="请输入知识标题"
                value={knowledge.title}
                onChange={(event) => formChange('title', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">分类</label>
              <Input
                placeholder="请输入分类，分类将会自动归集"
                value={knowledge.category}
                onChange={(event) => formChange('category', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">语言</label>
              <Select
                placeholder="请选择知识语言"
                defaultValue={knowledge.language || 1}
                style={{ width: '100%' }}
                value={knowledge.language}
                onChange={(value) => formChange('language', value)}
              >
                {LEGACY_KNOWLEDGE_LOCALES.map((locale) => (
                  <Select.Option value={locale}>
                    {LEGACY_KNOWLEDGE_I18N_TEXT[locale]}
                  </Select.Option>
                ))}
              </Select>
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">内容</label>
              <LegacyMarkdownEditor
                key={editorKey}
                value={knowledge.body}
                onChange={(value) => formChange('body', value)}
              />
            </div>
          </div>
        )}
        <div className="v2board-drawer-action">
          <Button style={{ marginRight: 8 }} onClick={hide}>
            取消
          </Button>
          <Button loading={saveLoading} onClick={() => void save()} type="primary">
            提交
          </Button>
        </div>
      </Drawer>
    </>
  );
}

export default function KnowledgePage() {
  const list = useAdminKnowledge();
  useAdminKnowledgeCategories();
  const save = useSaveKnowledgeMutation();
  const drop = useDropKnowledgeMutation();
  const show = useShowKnowledgeMutation();
  const sort = useSortKnowledgeMutation();
  const [orderedKnowledge, setOrderedKnowledge] = useState<KnowledgeSummary[]>(() => list.data ?? []);
  const [sortingLoading, setSortingLoading] = useState(false);
  const orderRef = useRef(orderedKnowledge);
  const dragIndex = useRef<number | null>(null);

  useEffect(() => {
    if (list.data) {
      setOrderedKnowledge(list.data);
      setSortingLoading(false);
    }
  }, [list.data]);

  orderRef.current = orderedKnowledge;

  const components = useMemo(
    () => ({
      body: {
        row: (
          props: HTMLAttributes<HTMLTableRowElement> & { 'data-sort-index'?: number },
        ) => {
          const onDrop = () => {
            const from = dragIndex.current;
            const current = orderRef.current;
            const to = Number(props['data-sort-index']);
            dragIndex.current = null;
            if (from == null || !Number.isFinite(to) || from === to) return;

            const next = [...current];
            if (from < to) {
              next.splice(to + 1, 0, next[from] as KnowledgeSummary);
              next.splice(from, 1);
            } else {
              next.splice(to, 0, next[from] as KnowledgeSummary);
              next.splice(from + 1, 1);
            }
            setOrderedKnowledge(next);
            setSortingLoading(true);
            sort.mutate(next.map((knowledge) => knowledge.id), {
              onSuccess: () => {
                void list.refetch();
              },
            });
          };

          return <tr {...props} onDragOver={(event) => event.preventDefault()} onDrop={onDrop} />;
        },
      },
    }),
    [list, sort],
  );

  const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);
  const refetchKnowledge = () => {
    void list.refetch();
  };

  const columns: TableProps<KnowledgeSummary>['columns'] = [
    {
      title: '排序',
      dataIndex: 'sort',
      key: 'sort',
      render: (_value, _row, index) => (
        <MenuOutlined
          draggable
          onDragStart={() => {
            dragIndex.current = index;
          }}
          style={{ cursor: 'move' }}
        />
      ),
    },
    {
      title: '文章ID',
      dataIndex: 'id',
      key: 'id',
    },
    {
      title: '显示',
      dataIndex: 'show',
      key: 'show',
      render: (value: 0 | 1 | undefined, row) => (
        <Switch
          size="small"
          onChange={() =>
            show.mutate(row.id, {
              onSuccess: () => {
                void list.refetch();
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
      title: '分类',
      dataIndex: 'category',
      key: 'category',
    },
    {
      title: '更新时间',
      dataIndex: 'updated_at',
      key: 'updated_at',
      align: 'right',
      render: (value: number) => dayjs(1000 * value).format('YYYY/MM/DD HH:mm'),
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      align: 'right',
      fixed: 'right',
      render: (_value, row) => (
        <>
          <KnowledgeEditor id={row.id} onSave={saveKnowledge} onSaved={refetchKnowledge}>
            <a ref={legacyHref()}>编辑</a>
          </KnowledgeEditor>
          <div className="ant-divider ant-divider-vertical" />
          <a
            ref={legacyHref()}
            onClick={() => {
              Modal.confirm({
                title: '警告',
                content: '确定要删除该条项目吗？',
                onOk: () => {
                  void drop.mutateAsync(row.id).then(() => {
                    void list.refetch();
                  });
                },
                okText: '确定',
                cancelText: '取消',
              });
            }}
          >
            删除
          </a>
        </>
      ),
    },
  ];

  return (
    <LegacySpin loading={list.isFetching || sortingLoading}>
      <div className="block border-bottom">
        <div className="bg-white">
          <div style={{ padding: 15 }}>
            <KnowledgeEditor onSave={saveKnowledge} onSaved={refetchKnowledge}>
              <Button>
                <PlusOutlined /> 新增
              </Button>
            </KnowledgeEditor>
          </div>
          <Table<KnowledgeSummary>
            tableLayout="auto"
            dataSource={orderedKnowledge}
            pagination={false}
            columns={columns}
            components={components}
            onRow={(_record, index) =>
              ({ 'data-sort-index': index } as HTMLAttributes<HTMLElement>)
            }
            scroll={{ x: 750 }}
          />
        </div>
      </div>
    </LegacySpin>
  );
}
