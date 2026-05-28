import { useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { TransferDialog } from '@/components/dialogs/transfer-dialog';
import { WithdrawDialog } from '@/components/dialogs/withdraw-dialog';
import { LegacySelect } from '@/components/legacy-select';
import { LegacyEmpty } from '@/components/legacy-empty';
import { TransactionIcon, PayCircleIcon } from '@/components/ant-icon';
import {
  useGenerateInviteMutation,
  useInvite,
  useInviteDetails,
  useCommConfig,
  useUserInfo,
} from '@/lib/queries';
import { formatDateMinuteSlash } from '@v2board/config/format';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';

export default function InvitePage() {
  const { t } = useTranslation();
  const invite = useInvite();
  const userInfo = useUserInfo();
  const { data: comm } = useCommConfig();
  const generate = useGenerateInviteMutation();
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const details = useInviteDetails(page, pageSize);
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
  const commissionRate =
    rate === undefined
      ? undefined
      : isDistribution
        ? [
            comm?.commission_distribution_l1,
            comm?.commission_distribution_l2,
            comm?.commission_distribution_l3,
          ]
            .map((level) => `${Number(level) * (rate / 100)}%`)
            .join(',')
        : `${rate}%`;
  const loading = invite.isFetching;

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
                    <button type="button" className="ant-btn ant-btn-primary mr-2">
                      <TransactionIcon />
                      <span> {t('invite.transfer')}</span>
                    </button>
                  </TransferDialog>
                  {!comm?.withdraw_close && (
                    <WithdrawDialog methods={comm?.withdraw_methods ?? []}>
                      <button type="button" className="ant-btn">
                        <PayCircleIcon />
                        <span> {t('invite.withdraw_button')}</span>
                      </button>
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
                value={registered !== undefined ? `${registered}人` : (
                  <>
                    <LegacyLoadingIcon />
                    人
                  </>
                )}
              />
              <StatRow
                label={
                  isDistribution ? (
                    <>
                      {t('invite.triple_rate')}{' '}
                      <span
                        className="anticon anticon-question-circle v2board-ant-tooltip-trigger"
                        data-title={t('invite.triple_hint')}
                      />
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
                    <span
                      className="anticon anticon-question-circle v2board-ant-tooltip-trigger"
                      data-title={t('invite.pending_hint')}
                    />
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
                      toast.success(t('invite.generated'));
                    } catch {}
                  }}
                >
                  {generate.isPending ? <LegacyLoadingIcon /> : t('invite.generate')}
                </button>
              </div>
            </div>
            <div className="block-content p-0">
              <div className="ant-table-wrapper">
                <div className={`ant-table ant-table-default ${codes.length ? '' : 'ant-table-empty'}`}>
                  <div className="ant-table-content">
                    <div className="ant-table-body">
                      <table style={{ width: '100%', tableLayout: 'auto' }}>
                        <thead className="ant-table-thead">
                          <tr>
                            <th className="ant-table-cell">{t('invite.code_col')}</th>
                            <th className="ant-table-cell ant-table-cell-align-right">
                              {t('invite.created_at_col')}
                            </th>
                          </tr>
                        </thead>
                        <tbody className="ant-table-tbody">
                          {codes.length ? (
                            codes.map((code) => (
                              <tr className="ant-table-row ant-table-row-level-0" key={code.id}>
                                <td className="ant-table-cell">
                                  <span>{code.code}</span>
                                  <a
                                    style={{ marginLeft: 5 }}
                                    href="javascript:void(0);"
                                    onClick={() => void copyInviteLink(code.code)}
                                  >
                                    {t('invite.invite_link')}
                                  </a>
                                </td>
                                <td className="ant-table-cell ant-table-cell-align-right">
                                  {formatDateMinuteSlash(code.created_at)}
                                </td>
                              </tr>
                            ))
                          ) : (
                            <tr className="ant-table-placeholder">
                              <td className="ant-table-cell" colSpan={2}>
                                <LegacyEmpty />
                              </td>
                            </tr>
                          )}
                        </tbody>
                      </table>
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
                <div
                  className={`ant-table ant-table-default ${
                    details.data?.data.length ? '' : 'ant-table-empty'
                  } ${details.isFetching ? 'ant-table-loading' : ''}`}
                >
                  <div className="ant-table-content">
                    <div className="ant-table-body">
                      <table style={{ width: '100%', tableLayout: 'auto' }}>
                        <thead className="ant-table-thead">
                          <tr>
                            <th className="ant-table-cell">{t('invite.issued_at')}</th>
                            <th className="ant-table-cell ant-table-cell-align-right">
                              {t('invite.commission_col')}
                            </th>
                          </tr>
                        </thead>
                        <tbody className="ant-table-tbody">
                          {details.data?.data.length ? (
                            details.data.data.map((row) => (
                              <tr className="ant-table-row ant-table-row-level-0" key={row.id}>
                                <td className="ant-table-cell">{formatDateMinuteSlash(row.created_at)}</td>
                                <td className="ant-table-cell ant-table-cell-align-right">
                                  {(row.get_amount / 100).toFixed(2)}
                                </td>
                              </tr>
                            ))
                          ) : (
                            <tr className="ant-table-placeholder">
                              <td className="ant-table-cell" colSpan={2}>
                                <LegacyEmpty />
                              </td>
                            </tr>
                          )}
                        </tbody>
                      </table>
                    </div>
                  </div>
                  {details.isFetching && (
                    <div className="ant-table-spin-holder">
                      <LegacyLoadingIcon />
                    </div>
                  )}
                </div>
              </div>
              {details.data && details.data.total > 0 && (
                <InvitePagination
                  current={page}
                  pageSize={pageSize}
                  total={details.data.total}
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
  return (parseInt(String(cents), 10) / 100).toFixed(2);
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
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const items = getPaginationItems(current, totalPages);

  return (
    <ul className="ant-table-pagination ant-pagination mini">
      <li className={`ant-pagination-prev ${current <= 1 ? 'ant-pagination-disabled' : ''}`}>
        <button
          type="button"
          className="ant-pagination-item-link"
          disabled={current <= 1}
          onClick={() => onChange(Math.max(1, current - 1), pageSize)}
        >
          <i className="fa fa-angle-left" />
        </button>
      </li>
      {items.map((item) =>
        typeof item === 'number' ? (
          <li
            key={item}
            className={`ant-pagination-item ant-pagination-item-${item} ${
              item === current ? 'ant-pagination-item-active' : ''
            }`}
          >
            <a onClick={() => onChange(item, pageSize)}>{item}</a>
          </li>
        ) : (
          <li
            key={item}
            className={`ant-pagination-jump-${item === 'jump-prev' ? 'prev' : 'next'}`}
            onClick={() =>
              onChange(item === 'jump-prev' ? Math.max(1, current - 5) : Math.min(totalPages, current + 5), pageSize)
            }
          >
            <div className="ant-pagination-item-container">
              <span className="ant-pagination-item-ellipsis">...</span>
            </div>
          </li>
        ),
      )}
      <li className={`ant-pagination-next ${current >= totalPages ? 'ant-pagination-disabled' : ''}`}>
        <button
          type="button"
          className="ant-pagination-item-link"
          disabled={current >= totalPages}
          onClick={() => onChange(Math.min(totalPages, current + 1), pageSize)}
        >
          <i className="fa fa-angle-right" />
        </button>
      </li>
      <li className="ant-pagination-options">
        <LegacySelect
          className="ant-pagination-options-size-changer"
          value={String(pageSize)}
          options={[10, 50, 100, 150].map((size) => ({
            value: String(size),
            label: `${size} 条/页`,
          }))}
          onChange={(value) => onChange(1, Number(value))}
        />
      </li>
    </ul>
  );
}

function getPaginationItems(current: number, totalPages: number): Array<number | 'jump-prev' | 'jump-next'> {
  if (totalPages <= 7) return Array.from({ length: totalPages }, (_, index) => index + 1);

  const left = Math.max(2, current - 2);
  const right = Math.min(totalPages - 1, current + 2);
  const items: Array<number | 'jump-prev' | 'jump-next'> = [1];

  if (left > 2) items.push('jump-prev');
  for (let page = left; page <= right; page += 1) items.push(page);
  if (right < totalPages - 1) items.push('jump-next');
  items.push(totalPages);

  return items;
}
