import { useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { TransferDialog } from '@/components/dialogs/transfer-dialog';
import { WithdrawDialog } from '@/components/dialogs/withdraw-dialog';
import { LegacyEmpty } from '@/components/legacy-empty';
import { LegacySelect } from '@/components/legacy-select';
import { LegacyTooltip } from '@/components/legacy-tooltip';
import { AntBtn } from '@/components/ant-btn';
import { legacyHref } from '@/lib/legacy-href';
import {
  TransactionIcon,
  PayCircleIcon,
  QuestionCircleIcon,
  LeftIcon,
  RightIcon,
  DoubleLeftIcon,
  DoubleRightIcon,
} from '@/components/ant-icon';
import {
  useGenerateInviteMutation,
  useInvite,
  useInviteDetails,
  useCommConfig,
  useUserInfo,
  userKeys,
} from '@/lib/queries';
import { formatUserLegacyDateMinuteSlash } from '@/lib/legacy-date';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

export default function InvitePage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  // Old componentDidMount dispatch order: user/getUserInfo, invite/details,
  // invite/fetch, then comm/config.
  const userInfo = useUserInfo({ refetchOnMount: 'always' });
  const [page, setPage] = useState<number | undefined>();
  const [pageSize, setPageSize] = useState<number | undefined>();
  const details = useInviteDetails(page, pageSize);
  const invite = useInvite();
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const generate = useGenerateInviteMutation();
  const symbol = comm?.currency_symbol;

  const stat = invite.data?.stat;
  const registered = stat?.[0];
  const validCommission = stat?.[1];
  const pendingCommission = stat?.[2];
  const rate = stat?.[3];
  const available = userInfo.data?.commission_balance;
  const availableText =
    userInfo.data?.commission_balance !== undefined
      ? formatCentsPlain(userInfo.data.commission_balance)
      : '--.--';
  const codes = invite.data?.codes ?? [];
  const isDistribution = Boolean(comm?.commission_distribution_enable);
  // Faithful to the original: the distribution branch computes the rate
  // unconditionally (l1 * (rate/100), …), so during the load window where comm config
  // has arrived but invite stats have not, rate is undefined → "NaN%,NaN%,NaN%".
  // Only the non-distribution branch guards (rate !== undefined ? `${rate}%` : loading).
  const commissionRate = isDistribution
    ? [
        comm?.commission_distribution_l1,
        comm?.commission_distribution_l2,
        comm?.commission_distribution_l3,
      ]
        .map((level) => `${Number(level) * (Number(rate) / 100)}%`)
        .join(',')
    : rate === undefined
      ? undefined
      : `${rate}%`;
  const loading = invite.isFetching;
  const detailRows = details.data?.data ?? [];
  const detailPaginationTotal = details.data?.total ?? detailRows.length;
  const detailPaginationItemTotal = detailPaginationTotal || detailRows.length;
  const detailPaginationCurrent = getLegacyMaxCurrent(
    detailPaginationItemTotal,
    page ?? 1,
    pageSize ?? 10,
  );
  const detailsLoading = useLegacyFetchLoading(details.isFetching);

  const copyInviteLink = (code: string) => {
    legacyCopyText(`${window.location.origin}${window.location.pathname}#/register?code=${code}`);
    toast.success(t('dashboard.copy_success'));
  };

  return (
    <>
      <div className="row mb-3 mb-md-0">
        <div className="col-md-12">
          <div className={`block block-rounded js-appear-enabled ${loading ? 'block-mode-loading' : ''}`}>
            <div className="block-content pb-3">
              <i className="fa fa-user-plus fa-2x text-gray-light float-right" />
              <div className="pb-sm-3">
                <p className="text-muted w-75">{t('invite.title')}</p>
                <p className="display-4 text-black font-w300 mb-2">
                  {availableText}
                  <span className="font-size-h5 text-muted ml-4">{comm?.currency}</span>
                </p>
                <span className="text-muted" style={{ cursor: 'pointer' }}>
                  {t('invite.available')}
                </span>
                <div className="pt-3">
                  <TransferDialog max={available}>
                    <AntBtn type="button" className="ant-btn ant-btn-primary mr-2">
                      <TransactionIcon />
                      <span> {t('invite.transfer')}</span>
                    </AntBtn>
                  </TransferDialog>
                  {!comm?.withdraw_close && (
                    <WithdrawDialog methods={comm?.withdraw_methods ?? []}>
                      <AntBtn type="button" className="ant-btn">
                        <PayCircleIcon />
                        <span> {t('invite.withdraw_button')}</span>
                      </AntBtn>
                    </WithdrawDialog>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="row mb-3 mb-md-0">
        <div className="col-md-12">
          <div className={`block block-rounded js-appear-enabled ${loading ? 'block-mode-loading' : ''}`}>
            <div className="block-content pb-3">
              <StatRow
                label={t('invite.registered')}
                value={(
                  <>
                    {registered !== undefined ? registered : <LegacyLoadingIcon />}
                    人
                  </>
                )}
              />
              <StatRow
                label={
                  isDistribution ? (
                    <>
                      {t('invite.triple_rate')}{' '}
                      <LegacyTooltip title={t('invite.triple_hint')}>
                        <QuestionCircleIcon />
                      </LegacyTooltip>
                    </>
                  ) : (
                    t('invite.commission_rate')
                  )
                }
                value={commissionRate}
              />
              <StatRow
                label={
                  <>
                    {t('invite.pending_commission')}{' '}
                    <LegacyTooltip title={t('invite.pending_hint')}>
                      <QuestionCircleIcon />
                    </LegacyTooltip>
                  </>
                }
                value={pendingCommission !== undefined ? `${symbol} ${pendingCommission / 100}` : undefined}
              />
              <StatRow
                label={t('invite.valid_commission')}
                value={validCommission !== undefined ? `${symbol} ${validCommission / 100}` : undefined}
              />
            </div>
          </div>
        </div>
      </div>

      <div className="row mb-3 mb-md-0">
        <div className="col-md-12">
          <div className={`block block-rounded js-appear-enabled ${loading ? 'block-mode-loading' : ''}`}>
            <div className="block-header block-header-default">
              <h3 className="block-title">{t('invite.manage')}</h3>
              <div className="block-options">
                <button
                  type="button"
                  className="btn btn-primary btn-sm btn-primary btn-rounded px-3"
                  onClick={async () => {
                    if (generate.isPending) return;
                    try {
                      await generate.mutateAsync();
                      toast.success('已生成');
                      void queryClient.invalidateQueries({ queryKey: userKeys.invite, exact: true });
                    } catch {}
                  }}
                >
                  {generate.isPending ? <LegacyLoadingIcon /> : t('invite.generate')}
                </button>
              </div>
            </div>
            <div className="block-content p-0">
              <div className="ant-table-wrapper">
                {/* antd v3 Table always wraps its content in Spin (loading defaults to
                    false); with no loading prop the codes table never spins, so the
                    spinner div / ant-spin-blur are absent — only the two static wrappers. */}
                <div className="ant-spin-nested-loading">
                  <div className="ant-spin-container">
                    <div
                      className={[
                        'ant-table',
                        'ant-table-default',
                        codes.length ? '' : 'ant-table-empty',
                        'ant-table-scroll-position-left',
                      ].filter(Boolean).join(' ')}
                    >
                      <div className="ant-table-content">
                        <div className="ant-table-body">
                          <table className="">
                            <colgroup>
                              <col />
                              <col />
                            </colgroup>
                            <thead className="ant-table-thead">
                              <tr>
                                <th className="">
                                  <span className="ant-table-header-column">
                                    <div>
                                      <span className="ant-table-column-title">{t('invite.code_col')}</span>
                                      <span className="ant-table-column-sorter" />
                                    </div>
                                  </span>
                                </th>
                                <th
                                  className="ant-table-align-right ant-table-row-cell-last"
                                  style={{ textAlign: 'right' }}
                                >
                                  <span className="ant-table-header-column">
                                    <div>
                                      <span className="ant-table-column-title">{t('invite.created_at_col')}</span>
                                      <span className="ant-table-column-sorter" />
                                    </div>
                                  </span>
                                </th>
                              </tr>
                            </thead>
                            <tbody className="ant-table-tbody">
                              {codes.map((code, index) => (
                                <tr
                                  className="ant-table-row ant-table-row-level-0"
                                  data-row-key={index}
                                  key={index}
                                >
                                  <td className="">
                                    <span>{code.code}</span>
                                    <a
                                      style={{ marginLeft: 5 }}
                                      ref={legacyHref()}
                                      onClick={() => void copyInviteLink(code.code)}
                                    >
                                      {t('invite.invite_link')}
                                    </a>
                                  </td>
                                  <td className="" style={{ textAlign: 'right' }}>
                                    {formatUserLegacyDateMinuteSlash(code.created_at)}
                                  </td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                        {codes.length === 0 && (
                          <div className="ant-table-placeholder">
                            <LegacyEmpty />
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="row mb-3 mb-md-0">
        <div className="col-md-12">
          <div className={`block block-rounded js-appear-enabled ${loading ? 'block-mode-loading' : ''}`}>
            <div className="block-header block-header-default">
              <h3 className="block-title">{t('invite.history')}</h3>
            </div>
            <div className="block-content p-0">
              <div className="ant-table-wrapper">
                <div className="ant-spin-nested-loading">
                  {detailsLoading && (
                    <div>
                      <div className="ant-spin ant-spin-spinning">
                        <span className="ant-spin-dot ant-spin-dot-spin">
                          <i className="ant-spin-dot-item" />
                          <i className="ant-spin-dot-item" />
                          <i className="ant-spin-dot-item" />
                          <i className="ant-spin-dot-item" />
                        </span>
                      </div>
                    </div>
                  )}
                  <div
                    className={`ant-spin-container${detailsLoading ? ' ant-spin-blur' : ''}`}
                  >
                    <div
                      className={[
                        'ant-table',
                        'ant-table-default',
                        detailRows.length ? '' : 'ant-table-empty',
                        'ant-table-scroll-position-left',
                      ].filter(Boolean).join(' ')}
                    >
                      <div className="ant-table-content">
                        <div className="ant-table-body">
                          <table className="">
                            <colgroup>
                              <col />
                              <col />
                            </colgroup>
                            <thead className="ant-table-thead">
                              <tr>
                                <th className="">
                                  <span className="ant-table-header-column">
                                    <div>
                                      <span className="ant-table-column-title">{t('invite.issued_at')}</span>
                                      <span className="ant-table-column-sorter" />
                                    </div>
                                  </span>
                                </th>
                                <th
                                  className="ant-table-align-right ant-table-row-cell-last"
                                  style={{ textAlign: 'right' }}
                                >
                                  <span className="ant-table-header-column">
                                    <div>
                                      <span className="ant-table-column-title">{t('invite.commission_col')}</span>
                                      <span className="ant-table-column-sorter" />
                                    </div>
                                  </span>
                                </th>
                              </tr>
                            </thead>
                            <tbody className="ant-table-tbody">
                              {detailRows.map((row, index) => (
                                <tr
                                  className="ant-table-row ant-table-row-level-0"
                                  data-row-key={index}
                                  key={index}
                                >
                                  <td className="">
                                    {formatUserLegacyDateMinuteSlash(row.created_at)}
                                  </td>
                                  <td className="" style={{ textAlign: 'right' }}>
                                    {(row.get_amount / 100).toFixed(2)}
                                  </td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                        {!detailRows.length && (
                          <div className="ant-table-placeholder">
                            <LegacyEmpty />
                          </div>
                        )}
                      </div>
                    </div>
                    {detailPaginationItemTotal > 0 && (
                      <InvitePagination
                        current={detailPaginationCurrent}
                        pageSize={pageSize ?? 10}
                        total={detailPaginationItemTotal}
                        onChange={(nextPage, nextPageSize) => {
                          setPage(nextPage);
                          setPageSize(nextPageSize);
                        }}
                      />
                    )}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

function StatRow({ label, value }: { label: ReactNode; value?: ReactNode }) {
  return (
    <div style={{ display: 'flex', padding: '5px 0' }}>
      <div style={{ flex: 1 }}>{label}</div>
      <div style={{ flex: 1, textAlign: 'right' }}>{value ?? <LegacyLoadingIcon />}</div>
    </div>
  );
}

function formatCentsPlain(cents: number) {
  return (parseInt(String(cents)) / 100).toFixed(2);
}

function getLegacyMaxCurrent(total: number, current: number, pageSize: number) {
  return (current - 1) * pageSize >= total ? Math.floor((total - 1) / pageSize) + 1 : current;
}

function InvitePagination({
  current,
  pageSize,
  total,
  onChange,
}: {
  current: number;
  pageSize: number;
  total: number;
  onChange: (page: number, pageSize: number) => void;
}) {
  const { t } = useTranslation();
  // rc-pagination's page count is Math.floor((total - 1) / pageSize) + 1,
  // so total=0 renders its disabled "0" pager instead of an active page 1.
  const totalPages = Math.floor((total - 1) / pageSize) + 1;
  const items = getPaginationItems(current, totalPages);
  const runIfEnter = (event: { key?: string; charCode?: number }, action: () => void) => {
    if (event.key === 'Enter' || event.charCode === 13) action();
  };
  const jumpPage = (item: 'jump-prev' | 'jump-next') =>
    item === 'jump-prev' ? Math.max(1, current - 5) : Math.min(totalPages, current + 5);
  const changePage = (targetPage: number) => {
    let nextPage = targetPage;
    if (nextPage > totalPages) nextPage = totalPages;
    if (nextPage < 1) nextPage = 1;
    onChange(nextPage, pageSize);
  };
  const goPrev = () => {
    if (current > 1) onChange(current - 1, pageSize);
  };
  const goNext = () => {
    if (current < totalPages) onChange(current + 1, pageSize);
  };

  return (
    <ul
      // rc-pagination stamps the legacy IE `unselectable="unselectable"` on its <ul>;
      // React's DOM types only allow "on"/"off", so set the exact attribute via a ref.
      ref={(node) => {
        if (node) node.setAttribute('unselectable', 'unselectable');
      }}
      className="ant-table-pagination ant-pagination mini"
    >
      <li
        title={t('common.prev_page')}
        className={`ant-pagination-prev ${current <= 1 ? 'ant-pagination-disabled' : ''}`}
        aria-disabled={current <= 1}
        tabIndex={current <= 1 ? undefined : 0}
        onClick={goPrev}
        // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
        onKeyPress={(event) => runIfEnter(event, goPrev)}
      >
        <a className="ant-pagination-item-link">
          <LeftIcon />
        </a>
      </li>
      {items.map((item, index) =>
        typeof item === 'number' ? (
          <li
            key={item}
            title={String(item)}
            tabIndex={0}
            // antd marks the windowed item adjacent to each ellipsis so the media
            // query at ≤992px (`.ant-pagination-item-{after-jump-prev,before-jump-next}`)
            // can hide it on narrow screens.
            className={[
              'ant-pagination-item',
              `ant-pagination-item-${item}`,
              item === current && 'ant-pagination-item-active',
              item === 0 && 'ant-pagination-disabled ant-pagination-item-disabled',
              items[index - 1] === 'jump-prev' && 'ant-pagination-item-after-jump-prev',
              items[index + 1] === 'jump-next' && 'ant-pagination-item-before-jump-next',
            ]
              .filter(Boolean)
              .join(' ')}
            onClick={() => changePage(item)}
            // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
            onKeyPress={(event) => runIfEnter(event, () => changePage(item))}
          >
            <a>{item}</a>
          </li>
        ) : (
          <li
            key={item}
            title={item === 'jump-prev' ? t('common.prev_5') : t('common.next_5')}
            tabIndex={0}
            className={`ant-pagination-jump-${item === 'jump-prev' ? 'prev' : 'next'}`}
            onClick={() => onChange(jumpPage(item), pageSize)}
            // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
            onKeyPress={(event) => runIfEnter(event, () => onChange(jumpPage(item), pageSize))}
          >
            <a className="ant-pagination-item-link">
              <div className="ant-pagination-item-container">
                {item === 'jump-prev' ? (
                  <DoubleLeftIcon className="ant-pagination-item-link-icon" />
                ) : (
                  <DoubleRightIcon className="ant-pagination-item-link-icon" />
                )}
                <span className="ant-pagination-item-ellipsis">•••</span>
              </div>
            </a>
          </li>
        ),
      )}
      <li
        title={t('common.next_page')}
        className={`ant-pagination-next ${current >= totalPages ? 'ant-pagination-disabled' : ''}`}
        aria-disabled={current >= totalPages}
        tabIndex={current >= totalPages ? undefined : 0}
        onClick={goNext}
        // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
        onKeyPress={(event) => runIfEnter(event, goNext)}
      >
        <a className="ant-pagination-item-link">
          <RightIcon />
        </a>
      </li>
      <li className="ant-pagination-options">
        <LegacySelect
          // size="small" Pagination → ant-select-sm. rc-pagination feeds the Select a
          // string value (`String(pageSize)`) but keys its options by the numeric
          // pageSizeOptions, so antd never matches the two: the trigger shows the bare
          // size while the menu shows "N 条/页".
          className="ant-pagination-options-size-changer"
          size="small"
          value={String(pageSize)}
          dropdownMatchSelectWidth={false}
          getPopupContainer={(trigger) => trigger.parentElement}
          options={[10, 50, 100, 150].map((size) => ({
            value: size,
            label: `${size} ${t('common.items_per_page')}`,
          }))}
          onChange={(value) => {
            const ps = Number.parseInt(String(value), 10);
            const nextTotalPages = Math.floor((total - 1) / ps) + 1;
            onChange(nextTotalPages === 0 ? current : Math.min(current, nextTotalPages), ps);
          }}
        />
      </li>
    </ul>
  );
}

function getPaginationItems(current: number, totalPages: number): Array<number | 'jump-prev' | 'jump-next'> {
  if (totalPages === 0) return [0];
  if (totalPages <= 9) return Array.from({ length: totalPages }, (_, index) => index + 1);

  let left = Math.max(2, current - 2);
  let right = Math.min(totalPages - 1, current + 2);
  if (current - 1 <= 2) right = 5;
  if (totalPages - current <= 2) left = totalPages - 4;
  const items: Array<number | 'jump-prev' | 'jump-next'> = [1];

  if (left > 2) items.push('jump-prev');
  for (let page = left; page <= right; page += 1) items.push(page);
  if (right < totalPages - 1) items.push('jump-next');
  items.push(totalPages);

  return items;
}
