import { cloneElement, useEffect, useMemo, useRef, useState, type ReactElement } from 'react';
import { App } from 'antd';
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
import { LegacyDragSort, LegacyMenuIcon } from '@/components/legacy-drag-sort';
import { LegacyButton } from '@/components/legacy-button';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyLoadingIcon, LegacyPlusIcon } from '@/components/legacy-ant-icon';
import { LegacySelect } from '@/components/legacy-select';
import { LegacyInput } from '@/components/legacy-input';
import { LegacyDrawer } from '@/components/legacy-drawer';
import {
  LegacyStandaloneTable,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
} from '@/components/legacy-standalone-table';

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
const LEGACY_KNOWLEDGE_LOCALE_OPTIONS = LEGACY_KNOWLEDGE_LOCALES.map((locale) => ({
  value: locale,
  label: LEGACY_KNOWLEDGE_I18N_TEXT[locale],
}));

function renderLegacyAdminMarkdown(markdown: string) {
  return legacyAdminMarkdown.render(markdown);
}

function LegacyKnowledgeSwitch({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  const enabled = Boolean(checked);

  return (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      className={`ant-switch-small ant-switch${enabled ? ' ant-switch-checked' : ''}`}
      onClick={onChange}
    >
      <span className="ant-switch-inner" />
    </button>
  );
}

const LEGACY_TABLE_ROWS = 4;
const LEGACY_TABLE_COLS = 6;
const LEGACY_TABLE_CELL_GAP = 3;
const LEGACY_TABLE_CELL_STEP = 23;

type LegacyMarkdownView = { md: boolean; html: boolean };
type LegacyHeaderTag = `h${1 | 2 | 3 | 4 | 5 | 6}`;
type LegacySelection = { start: number; end: number; selected: string };

function legacyTableMarkdown(row: number, col: number) {
  const cells = (label: string) => Array.from({ length: col }, () => ` ${label} |`).join('');
  const rows = Array.from({ length: row }, () => `|${cells('Data')}`).join('\n');

  return `\n|${cells('Head')}\n|${cells('---')}\n${rows}\n`;
}

function legacyListMarkdown(type: 'ordered' | 'unordered', selected: string) {
  const rows = selected ? selected.split('\n') : [''];
  return rows
    .map((line, index) => `${type === 'ordered' ? `${index + 1}. ` : '* '}${line}`)
    .join('\n');
}

function LegacyMarkdownEditor({
  value,
  onChange,
}: {
  value?: string;
  onChange: (value: string) => void;
}) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const imageInputRef = useRef<HTMLInputElement | null>(null);
  const [view, setView] = useState<LegacyMarkdownView>({ md: true, html: true });
  const [fullScreen, setFullScreen] = useState(false);
  const [headerMenuVisible, setHeaderMenuVisible] = useState(false);
  const [tableMenuVisible, setTableMenuVisible] = useState(false);
  const [tableHover, setTableHover] = useState<{ row: number; col: number } | null>(null);
  const [undoStack, setUndoStack] = useState<string[]>([]);
  const [redoStack, setRedoStack] = useState<string[]>([]);
  const text = value ?? '';
  const html = useMemo(() => renderLegacyAdminMarkdown(text), [text]);

  const nextViewInfo = () => {
    if (view.md && view.html) {
      return { view: { md: true, html: false }, icon: 'keyboard', title: 'Only display editor' };
    }
    if (view.md) {
      return { view: { md: false, html: true }, icon: 'visibility', title: 'Only display preview' };
    }
    return {
      view: { md: true, html: true },
      icon: 'view-split',
      title: 'Display both editor and preview',
    };
  };

  const getSelection = (): LegacySelection => {
    const textarea = textareaRef.current;
    const start = textarea?.selectionStart ?? text.length;
    const end = textarea?.selectionEnd ?? text.length;
    return { start, end, selected: text.slice(start, end) };
  };

  const applyTextChange = (nextText: string) => {
    if (nextText === text) return;
    setUndoStack((stack) => [...stack, text].slice(-100));
    setRedoStack([]);
    onChange(nextText);
  };

  const replaceSelection = (replacement: string, selection = getSelection()) => {
    applyTextChange(`${text.slice(0, selection.start)}${replacement}${text.slice(selection.end)}`);
  };

  const wrapSelection = (before: string, after = before) => {
    const selection = getSelection();
    replaceSelection(`${before}${selection.selected}${after}`, selection);
  };

  const insertMarkdownBlock = (before: string, after = '') => {
    const selection = getSelection();
    replaceSelection(`${before}${selection.selected}${after}`, selection);
  };

  const insertHeader = (tag: LegacyHeaderTag) => {
    insertMarkdownBlock(`\n${'#'.repeat(Number(tag.slice(1)))} `, '\n');
    setHeaderMenuVisible(false);
  };

  const insertTable = (row: number, col: number) => {
    replaceSelection(legacyTableMarkdown(row, col));
    setTableMenuVisible(false);
  };

  const insertImage = (label = '') => {
    const selection = getSelection();
    replaceSelection(`![${selection.selected || label}]()`, selection);
  };

  const bindImageInput = (node: HTMLInputElement | null) => {
    imageInputRef.current = node;
    if (!node) return;
    node.setAttribute('type', 'file');
    node.setAttribute('accept', '');
    node.setAttribute(
      'style',
      'position: absolute; z-index: -1; left: 0px; top: 0px; width: 0px; height: 0px; opacity: 0;',
    );
  };

  const undoMarkdown = () => {
    setUndoStack((stack) => {
      const previous = stack[stack.length - 1];
      if (previous === undefined) return stack;
      setRedoStack((redo) => [...redo, text].slice(-100));
      onChange(previous);
      return stack.slice(0, -1);
    });
  };

  const redoMarkdown = () => {
    setRedoStack((stack) => {
      const next = stack[stack.length - 1];
      if (next === undefined) return stack;
      setUndoStack((undo) => [...undo, text].slice(-100));
      onChange(next);
      return stack.slice(0, -1);
    });
  };

  const mode = nextViewInfo();

  return (
    <div className={`rc-md-editor ${fullScreen ? 'full' : ''} `} style={{ height: 500 }}>
      <div className="rc-md-navigation visible">
        <div className="navigation-nav left">
          <div className="button-wrap">
            <span
              className="button button-type-header"
              title="Header"
              onMouseEnter={() => setHeaderMenuVisible(true)}
              onMouseLeave={() => setHeaderMenuVisible(false)}
            >
              <i className="rmel-iconfont rmel-icon-font-size" />
              <div
                className={`drop-wrap ${headerMenuVisible ? 'show' : 'hidden'}`}
                onClick={(event) => {
                  event.stopPropagation();
                  setHeaderMenuVisible(false);
                }}
              >
                <ul className="header-list">
                  <li className="list-item">
                    <h1 onClick={() => insertHeader('h1')}>H1</h1>
                  </li>
                  <li className="list-item">
                    <h2 onClick={() => insertHeader('h2')}>H2</h2>
                  </li>
                  <li className="list-item">
                    <h3 onClick={() => insertHeader('h3')}>H3</h3>
                  </li>
                  <li className="list-item">
                    <h4 onClick={() => insertHeader('h4')}>H4</h4>
                  </li>
                  <li className="list-item">
                    <h5 onClick={() => insertHeader('h5')}>H5</h5>
                  </li>
                  <li className="list-item">
                    <h6 onClick={() => insertHeader('h6')}>H6</h6>
                  </li>
                </ul>
              </div>
            </span>
            <span
              className="button button-type-bold"
              title="Bold"
              onClick={() => wrapSelection('**')}
            >
              <i className="rmel-iconfont rmel-icon-bold" />
            </span>
            <span
              className="button button-type-italic"
              title="Italic"
              onClick={() => wrapSelection('*')}
            >
              <i className="rmel-iconfont rmel-icon-italic" />
            </span>
            <span
              className="button button-type-underline"
              title="Underline"
              onClick={() => wrapSelection('++')}
            >
              <i className="rmel-iconfont rmel-icon-underline" />
            </span>
            <span
              className="button button-type-strikethrough"
              title="Strikethrough"
              onClick={() => wrapSelection('~~')}
            >
              <i className="rmel-iconfont rmel-icon-strikethrough" />
            </span>
            <span
              className="button button-type-unordered"
              title="Unordered list"
              onClick={() => {
                const selection = getSelection();
                replaceSelection(legacyListMarkdown('unordered', selection.selected), selection);
              }}
            >
              <i className="rmel-iconfont rmel-icon-list-unordered" />
            </span>
            <span
              className="button button-type-ordered"
              title="Ordered list"
              onClick={() => {
                const selection = getSelection();
                replaceSelection(legacyListMarkdown('ordered', selection.selected), selection);
              }}
            >
              <i className="rmel-iconfont rmel-icon-list-ordered" />
            </span>
            <span
              className="button button-type-quote"
              title="Quote"
              onClick={() => insertMarkdownBlock('\n> ', '\n')}
            >
              <i className="rmel-iconfont rmel-icon-quote" />
            </span>
            <span
              className="button button-type-wrap"
              title="Line break"
              onClick={() => replaceSelection('\n')}
            >
              <i className="rmel-iconfont rmel-icon-wrap" />
            </span>
            <span
              className="button button-type-code-inline"
              title="Inline code"
              onClick={() => wrapSelection('`')}
            >
              <i className="rmel-iconfont rmel-icon-code" />
            </span>
            <span
              className="button button-type-code-block"
              title="Code"
              onClick={() => insertMarkdownBlock('\n```\n', '\n```\n')}
            >
              <i className="rmel-iconfont rmel-icon-code-block" />
            </span>
            <span
              className="button button-type-table"
              title="Table"
              onMouseEnter={() => setTableMenuVisible(true)}
              onMouseLeave={() => setTableMenuVisible(false)}
            >
              <i className="rmel-iconfont rmel-icon-grid" />
              <div className={`drop-wrap ${tableMenuVisible ? 'show' : 'hidden'}`}>
                <ul
                  className="table-list wrap"
                  style={{
                    width: LEGACY_TABLE_CELL_STEP * LEGACY_TABLE_COLS - LEGACY_TABLE_CELL_GAP,
                    height: LEGACY_TABLE_CELL_STEP * LEGACY_TABLE_ROWS - LEGACY_TABLE_CELL_GAP,
                  }}
                >
                  {Array.from({ length: LEGACY_TABLE_ROWS }).map((_, row) =>
                    Array.from({ length: LEGACY_TABLE_COLS }).map((__, col) => (
                      <li
                        key={`${row}-${col}`}
                        className={`list-item ${
                          tableHover && row <= tableHover.row && col <= tableHover.col
                            ? 'active'
                            : ''
                        }`}
                        style={{
                          top: LEGACY_TABLE_CELL_STEP * row,
                          left: LEGACY_TABLE_CELL_STEP * col,
                        }}
                        onMouseOver={() => setTableHover({ row, col })}
                        onClick={(event) => {
                          event.stopPropagation();
                          insertTable(row + 1, col + 1);
                        }}
                      />
                    )),
                  )}
                </ul>
              </div>
            </span>
            <span
              className="button button-type-image"
              title="Image"
              style={{ position: 'relative' }}
              onClick={() => insertImage()}
            >
              <i className="rmel-iconfont rmel-icon-image" />
              <input
                ref={bindImageInput}
                onChange={(event) => {
                  const file = event.currentTarget.files?.[0];
                  if (file) insertImage(file.name);
                  event.currentTarget.value = '';
                }}
              />
            </span>
            <span
              className="button button-type-link"
              title="Link"
              onClick={() => {
                const selection = getSelection();
                replaceSelection(`[${selection.selected}]()`, selection);
              }}
            >
              <i className="rmel-iconfont rmel-icon-link" />
            </span>
            <span
              className="button button-type-clear"
              title="Clear"
              onClick={() => applyTextChange('')}
            >
              <i className="rmel-iconfont rmel-icon-delete" />
            </span>
            <span
              className={`button button-type-undo ${undoStack.length ? '' : 'disabled'}`}
              title="Undo"
              onClick={undoMarkdown}
            >
              <i className="rmel-iconfont rmel-icon-undo" />
            </span>
            <span
              className={`button button-type-redo ${redoStack.length ? '' : 'disabled'}`}
              title="Redo"
              onClick={redoMarkdown}
            >
              <i className="rmel-iconfont rmel-icon-redo" />
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
              title={fullScreen ? 'Exit full screen' : 'Full screen'}
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
        <section className={`section sec-md ${view.md ? 'visible' : 'in-visible'}`}>
          <textarea
            ref={textareaRef}
            name="textarea"
            value={text}
            className="section-container input "
            wrap="hard"
            onChange={(event) => applyTextChange(event.target.value)}
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
  saveLoading,
}: {
  id?: number;
  children: ReactElement<{ onClick?: () => void }>;
  onSave: (payload: SaveKnowledgePayload) => Promise<unknown>;
  onSaved: () => void | Promise<unknown>;
  saveLoading?: boolean;
}) {
  const { message } = App.useApp();
  const [visible, setVisible] = useState(false);
  const [loading, setLoading] = useState(false);
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
    await onSave({ ...knowledge });
    void onSaved();
    message.success('保存成功');
  };

  return (
    <>
      {cloneElement(children, { onClick: show })}
      <LegacyDrawer
        width="80%"
        id="knowledge"
        open={visible}
        title={id ? '编辑知识' : '新增知识'}
        onClose={hide}
      >
        {loading ? (
          <LegacyLoadingIcon />
        ) : (
          <div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">标题</label>
              <LegacyInput
                key={`title-${editorKey}`}
                className="ant-input"
                placeholder="请输入知识标题"
                defaultValue={knowledge.title}
                onChange={(event) => formChange('title', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">分类</label>
              <LegacyInput
                key={`category-${editorKey}`}
                className="ant-input"
                placeholder="请输入分类，分类将会自动归集"
                defaultValue={knowledge.category}
                onChange={(event) => formChange('category', event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">语言</label>
              <LegacySelect
                placeholder="请选择知识语言"
                style={{ width: '100%' }}
                value={knowledge.language}
                options={LEGACY_KNOWLEDGE_LOCALE_OPTIONS}
                onChange={(value) => formChange('language', value)}
              />
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
          <LegacyButton className="ant-btn" style={{ marginRight: 8 }} onClick={hide}>
            取消
          </LegacyButton>
          <LegacyButton
            className={`ant-btn ant-btn-primary${saveLoading ? ' ant-btn-loading' : ''}`}
            onClick={() => void save()}
          >
            提交
          </LegacyButton>
        </div>
      </LegacyDrawer>
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
  const [orderedKnowledge, setOrderedKnowledge] = useState<KnowledgeSummary[]>(
    () => list.data ?? [],
  );
  const [sortingLoading, setSortingLoading] = useState(false);
  const orderRef = useRef(orderedKnowledge);

  useEffect(() => {
    if (list.data) {
      setOrderedKnowledge(list.data);
      setSortingLoading(false);
    }
  }, [list.data]);

  orderRef.current = orderedKnowledge;

  const sortKnowledge = (fromIndex: number, toIndex: number) => {
    const next = [...orderRef.current];
    const moved = next[fromIndex];
    if (!moved) return;
    if (fromIndex < toIndex) {
      next.splice(toIndex + 1, 0, moved);
      next.splice(fromIndex, 1);
    } else {
      next.splice(toIndex, 0, moved);
      next.splice(fromIndex + 1, 1);
    }
    setOrderedKnowledge(next);
    setSortingLoading(true);
    sort.mutate(
      next.map((knowledge) => knowledge.id),
      {
        onSuccess: () => {
          void list.refetch();
        },
      },
    );
  };

  const saveKnowledge = (payload: SaveKnowledgePayload) => save.mutateAsync(payload);
  const refetchKnowledge = () => list.refetch();

  const headers: LegacyStandaloneTableHeader[] = [
    { title: '排序' },
    { title: '文章ID' },
    { title: '显示' },
    { title: '标题' },
    { title: '分类' },
    { title: '更新时间', alignRight: true },
    { title: '操作', alignRight: true, fixedRight: true },
  ];

  const renderKnowledgeShowSwitch = (value: 0 | 1 | undefined, row: KnowledgeSummary) => (
    <LegacyKnowledgeSwitch
      onChange={() =>
        show.mutate(row.id, {
          onSuccess: () => {
            void list.refetch();
          },
        })
      }
      checked={value as unknown as boolean}
    />
  );

  const renderKnowledgeActions = (row: KnowledgeSummary) => (
    <>
      <KnowledgeEditor
        id={row.id}
        onSave={saveKnowledge}
        onSaved={refetchKnowledge}
        saveLoading={save.isPending}
      >
        <a ref={legacyHref()}>编辑</a>
      </KnowledgeEditor>
      <div className="ant-divider ant-divider-vertical" role="separator" />
      <a
        ref={legacyHref()}
        onClick={() => {
          void legacyConfirm({
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
  );

  return (
    <LegacySpin loading={list.isFetching || sortingLoading}>
      <div className="block border-bottom">
        <div className="bg-white">
          <div style={{ padding: 15 }}>
            <KnowledgeEditor
              onSave={saveKnowledge}
              onSaved={refetchKnowledge}
              saveLoading={save.isPending}
            >
              <LegacyButton className="ant-btn">
                <LegacyPlusIcon />
                <span>新增</span>
              </LegacyButton>
            </KnowledgeEditor>
          </div>
          <LegacyDragSort
            onDragEnd={(fromIndex, toIndex) => sortKnowledge(fromIndex, toIndex)}
            nodeSelector="tr"
            handleSelector="i"
          >
            <LegacyStandaloneTable
              headers={headers}
              isEmpty={orderedKnowledge.length === 0}
              scrollX={750}
              fixedRightChildren={orderedKnowledge.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  style={{ height: 54 }}
                  {...legacyTableRowKey(index)}
                >
                  <td className="" style={{ textAlign: 'right' }}>
                    {renderKnowledgeActions(row)}
                  </td>
                </tr>
              ))}
            >
              {orderedKnowledge.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyTableRowKey(index)}
                >
                  <td className="">
                    <LegacyMenuIcon style={{ cursor: 'move' }} />
                  </td>
                  <td className="">{row.id}</td>
                  <td className="">{renderKnowledgeShowSwitch(row.show, row)}</td>
                  <td className="">{row.title}</td>
                  <td className="">{row.category}</td>
                  <td className="" style={{ textAlign: 'right' }}>
                    {dayjs(1000 * row.updated_at).format('YYYY/MM/DD HH:mm')}
                  </td>
                  <td className="ant-table-fixed-columns-in-body" style={{ textAlign: 'right' }}>
                    {renderKnowledgeActions(row)}
                  </td>
                </tr>
              ))}
            </LegacyStandaloneTable>
          </LegacyDragSort>
        </div>
      </div>
    </LegacySpin>
  );
}
