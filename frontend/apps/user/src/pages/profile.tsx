import { useState } from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { user } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import {
  useChangePasswordMutation,
  useCommConfig,
  useRedeemGiftCardMutation,
  useResetSubscribeMutation,
  useSubscribe,
  useTelegramBotInfo,
  useUnbindTelegramMutation,
  useUpdateProfileMutation,
  useUserInfo,
} from '@/lib/queries';
import { AntBtn } from '@/components/ant-btn';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';

export default function ProfilePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const info = useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const subscribeQuery = useSubscribe();
  const subscribe = subscribeQuery.data;
  const updateProfile = useUpdateProfileMutation();
  const changePassword = useChangePasswordMutation();
  const redeem = useRedeemGiftCardMutation();
  const resetSub = useResetSubscribeMutation();
  const unbindTelegram = useUnbindTelegramMutation();

  const [giftCode, setGiftCode] = useState('');
  const [oldPwd, setOldPwd] = useState('');
  const [newPwd, setNewPwd] = useState('');
  const [confirmPwd, setConfirmPwd] = useState('');
  const [depositOpen, setDepositOpen] = useState(false);
  const [depositAmount, setDepositAmount] = useState('');
  const [telegramOpen, setTelegramOpen] = useState(false);
  const [updatingPref, setUpdatingPref] = useState<
    'auto_renewal' | 'remind_expire' | 'remind_traffic' | null
  >(null);
  const botInfo = useTelegramBotInfo(telegramOpen);

  const data = info.data;
  const currency = comm?.currency;

  const togglePref = async (
    key: 'auto_renewal' | 'remind_expire' | 'remind_traffic',
    value: 0 | 1,
  ) => {
    setUpdatingPref(key);
    try {
      await updateProfile.mutateAsync({ [key]: value } as Parameters<
        typeof updateProfile.mutateAsync
      >[0]);
    } catch {
    } finally {
      setUpdatingPref(null);
    }
  };

  const onChangePwd = async () => {
    if (newPwd !== confirmPwd) {
      toast.error(t('profile.password_mismatch'));
      return;
    }
    try {
      await changePassword.mutateAsync({ oldPassword: oldPwd, newPassword: newPwd });
      toast.success('修改成功，请重新登陆');
      navigate('/login');
    } catch {}
  };

  const onRedeem = async () => {
    if (giftCode.length === 0) {
      toast.error(t('profile.redeem_placeholder'));
      return;
    }
    try {
      const result = await redeem.mutateAsync(giftCode);
      toast.success(`兑换成功: ${redeemGiftcardText(result.type, result.value)}`);
    } catch {}
  };

  const onReset = async () => {
    const ok = await legacyConfirm({
      title: t('profile.reset_subscribe_confirm'),
      content: t('profile.reset_subscribe_tip'),
      okText: t('profile.confirm'),
      cancelText: t('common.cancel'),
    });
    if (!ok) return;
    try {
      await resetSub.mutateAsync();
      toast.success(t('profile.reset_success'));
      void info.refetch();
    } catch {}
  };

  const onUnbindTelegram = async () => {
    const ok = await legacyConfirm({
      title: t('profile.telegram_unbind_confirm'),
      content: t('profile.telegram_unbind_tip'),
      okText: t('profile.confirm'),
      cancelText: t('common.cancel'),
    });
    if (!ok) return;
    try {
      await unbindTelegram.mutateAsync();
      toast.success(t('profile.reset_success'));
      void info.refetch();
      void subscribeQuery.refetch();
    } catch {}
  };

  const onDeposit = async () => {
    try {
      const tradeNo = await user.saveOrder(apiClient, {
        plan_id: 0,
        period: 'deposit',
        deposit_amount: depositAmount === '' ? undefined : Number(depositAmount) * 100,
      });
      setDepositOpen(false);
      setDepositAmount('');
      navigate(`/order/${tradeNo}`);
    } catch {}
  };

  const copyBindCommand = () => {
    legacyCopyText(`/bind ${subscribe?.subscribe_url}`);
  };

  return (
    <>
      <div className="row mb-3 mb-md-0">
        <div className="col-lg-12">
          <div className="block ">
            <div className="block-content pb-3">
              <i className="fa fa-wallet fa-2x text-gray-light float-right" />
              <div className="pb-sm-3">
                <p className="text-muted w-75">{t('profile.wallet')}</p>
                <p className="display-4 text-black font-w300 mb-2">
                  {data?.balance !== undefined ? formatCentsPlain(data.balance) : '--.--'}
                  <span className="font-size-h5 text-muted ml-4">{currency}</span>
                </p>
                <span className="text-muted" style={{ cursor: 'pointer' }}>
                  {t('profile.auto_renewal')}{' '}
                  <LegacySwitch
                    loading={updatingPref === 'auto_renewal'}
                    checked={data?.auto_renewal === 1}
                    onChange={(checked) => void togglePref('auto_renewal', checked ? 1 : 0)}
                  />
                </span>
                <div className="pt-3">
                  <AntBtn type="button" className="ant-btn ant-btn-primary" onClick={() => setDepositOpen(true)}>
                    {t('profile.recharge')}
                  </AntBtn>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <LegacyBlock title={t('profile.redeem_giftcard')}>
        <div>
          <div className="row push">
            <div className="col-lg-8 col-xl-5">
              <div className="form-group">
                <input
                  className="form-control"
                  placeholder={t('profile.redeem_placeholder')}
                  autoComplete="one-time-code"
                  value={giftCode}
                  onChange={(event) => setGiftCode(event.target.value)}
                />
              </div>
              <AntBtn
                type="button"
                className="ant-btn ant-btn-primary"
                disabled={redeem.isPending}
                onClick={() => void onRedeem()}
              >
                {redeem.isPending ? <LegacyLoadingIcon /> : t('profile.redeem_submit')}
              </AntBtn>
            </div>
          </div>
        </div>
      </LegacyBlock>

      <LegacyBlock title={t('profile.change_password')}>
        <div>
          <div className="row push">
            <div className="col-lg-8 col-xl-5">
              <div className="form-group">
                <label htmlFor="profile-old-password">{t('profile.old_password')}</label>
                <input
                  id="profile-old-password"
                  type="password"
                  className="form-control"
                  placeholder={t('profile.old_password_placeholder')}
                  value={oldPwd}
                  onChange={(event) => setOldPwd(event.target.value)}
                />
              </div>
              <div className="form-group">
                <label htmlFor="profile-new-password">{t('profile.new_password')}</label>
                <input
                  id="profile-new-password"
                  type="password"
                  className="form-control"
                  placeholder={t('profile.new_password_placeholder')}
                  value={newPwd}
                  onChange={(event) => setNewPwd(event.target.value)}
                />
              </div>
              <div className="form-group">
                <label htmlFor="profile-confirm-password">{t('profile.new_password')}</label>
                <input
                  id="profile-confirm-password"
                  type="password"
                  className="form-control"
                  placeholder={t('profile.new_password_placeholder')}
                  value={confirmPwd}
                  onChange={(event) => setConfirmPwd(event.target.value)}
                />
              </div>
              <AntBtn
                type="button"
                className="ant-btn ant-btn-primary"
                disabled={changePassword.isPending}
                onClick={() => void onChangePwd()}
              >
                {changePassword.isPending ? <LegacyLoadingIcon /> : t('profile.save')}
              </AntBtn>
            </div>
          </div>
        </div>
      </LegacyBlock>

      <LegacyBlock title={t('profile.notifications')}>
        <div className="row">
          <div className="col-lg-8 col-xl-5">
            <div className="form-group">
              <label>{t('profile.remind_expire')}</label>
              <div>
                <LegacySwitch
                  loading={updatingPref === 'remind_expire'}
                  checked={data?.remind_expire === 1}
                  onChange={(checked) => void togglePref('remind_expire', checked ? 1 : 0)}
                />
              </div>
            </div>
            <div className="form-group">
              <label>{t('profile.remind_traffic')}</label>
              <div>
                <LegacySwitch
                  loading={updatingPref === 'remind_traffic'}
                  checked={data?.remind_traffic === 1}
                  onChange={(checked) => void togglePref('remind_traffic', checked ? 1 : 0)}
                />
              </div>
            </div>
          </div>
        </div>
      </LegacyBlock>

      <div className="row mb-3 mb-md-0">
        <div className="col-md-12">
          {comm?.is_telegram ? (
            !data?.telegram_id ? (
              <div className="block block-rounded bind_telegram">
                <div className="block-header block-header-default">
                  <h3 className="block-title">{t('profile.telegram_bind')}</h3>
                  <div className="block-options">
                    <button
                      type="button"
                      className="btn btn-primary btn-sm btn-primary btn-rounded px-3"
                      onClick={() => setTelegramOpen(true)}
                    >
                      {t('profile.start_now')}
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <div className="block block-rounded unbind_telegram">
                <div className="block-header block-header-default">
                  <h3 className="block-title">{t('profile.telegram_bind')}</h3>
                  <div className="block-options">
                    <AntBtn
                      type="button"
                      className="ant-btn ant-btn-danger"
                      onClick={onUnbindTelegram}
                    >
                      {t('profile.telegram_unbind')}
                    </AntBtn>
                  </div>
                </div>
                <div className="block-options">Telegram ID: {String(data.telegram_id)}</div>
              </div>
            )
          ) : null}

          {comm?.telegram_discuss_link ? (
            <div className="block block-rounded join_telegram_disscuss">
              <div className="block-header block-header-default">
                <h3 className="block-title">{t('profile.telegram_discuss')}</h3>
                <div className="block-options">
                  <a
                    href={comm.telegram_discuss_link}
                    target="_blank"
                    className="btn btn-primary btn-sm btn-primary btn-rounded px-3"
                  >
                    {t('profile.join_now')}
                  </a>
                </div>
              </div>
            </div>
          ) : null}

          <div className="block block-rounded">
            <div className="block-header block-header-default">
              <h3 className="block-title">{t('profile.reset_subscribe')}</h3>
              <div className="block-options" />
            </div>
            <div className="block-content">
              <div className="row push">
                <div className="col-md-12">
                  <div className="alert alert-warning mb-3" role="alert">
                    {t('profile.reset_subscribe_warning')}
                  </div>
                  <AntBtn
                    type="button"
                    className="ant-btn ant-btn-danger"
                    onClick={onReset}
                  >
                    {t('profile.reset')}
                  </AntBtn>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <Dialog open={depositOpen} onOpenChange={setDepositOpen}>
        <DialogContent
          className="v2board-ant-confirm-modal ant-modal-confirm ant-modal-confirm-confirm"
          showClose={false}
        >
          <div className="ant-modal-body">
            <div className="ant-modal-confirm-body-wrapper">
              <div className="ant-modal-confirm-body">
                <i className="anticon anticon-exclamation-circle" />
                <span className="ant-modal-confirm-title">
                  <input
                    className="form-control"
                    autoFocus
                    autoComplete="one-time-code"
                    placeholder={t('profile.deposit_placeholder', { currency })}
                    value={depositAmount}
                    onChange={(event) => setDepositAmount(event.target.value)}
                  />
                </span>
              </div>
              <div className="ant-modal-confirm-btns">
                <AntBtn type="button" className="ant-btn" onClick={() => setDepositOpen(false)}>
                  {t('common.cancel')}
                </AntBtn>
                <AntBtn type="button" className="ant-btn ant-btn-primary" onClick={() => void onDeposit()}>
                  {t('profile.confirm')}
                </AntBtn>
              </div>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={telegramOpen} onOpenChange={setTelegramOpen}>
        <DialogContent className="v2board-ant-modal">
          <div className="ant-modal-header">
            <div className="ant-modal-title">{t('profile.telegram_bind')}</div>
          </div>
          <div className="ant-modal-body">
            {botInfo.data?.username ? (
              <>
                <h2 className="content-heading pt-1">
                  <i className="fa fa-arrow-right text-info mr-1" /> {t('profile.telegram_step1')}
                </h2>
                <div>
                  {t('profile.telegram_search')}
                  <a href={`https://t.me/${botInfo.data.username}`}>@{botInfo.data.username}</a>
                </div>
                <h2 className="content-heading">
                  <i className="fa fa-arrow-right text-info mr-1" /> {t('profile.telegram_step2')}
                </h2>
                <div>
                  {t('profile.telegram_send')}
                  <br />
                  <code onClick={() => copyBindCommand()}>
                    /bind {subscribe?.subscribe_url ?? ''}
                  </code>
                </div>
              </>
            ) : (
              <LegacyLoadingIcon />
            )}
          </div>
          <div className="ant-modal-footer">
            <AntBtn type="button" className="ant-btn ant-btn-primary" onClick={() => setTelegramOpen(false)}>
              {t('profile.i_know')}
            </AntBtn>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

function LegacyBlock({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <div className="row mb-3 mb-md-0">
      <div className="col-md-12">
        <div className="block block-rounded">
          <div className="block-header block-header-default">
            <h3 className="block-title">{title}</h3>
            <div className="block-options" />
          </div>
          <div className="block-content">{children}</div>
        </div>
      </div>
    </div>
  );
}

function LegacySwitch({
  checked,
  loading,
  onChange,
}: {
  checked: boolean;
  loading?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      className={`ant-switch ${checked ? 'ant-switch-checked' : ''} ${loading ? 'ant-switch-loading' : ''}`}
      aria-checked={checked}
      role="switch"
      disabled={loading}
      onClick={() => onChange(!checked)}
    >
      {loading ? (
        <span className="ant-switch-loading-icon">
          <LegacyLoadingIcon />
        </span>
      ) : null}
      <span className="ant-switch-inner" />
    </button>
  );
}

function redeemGiftcardText(type: number, value: number) {
  switch (type) {
    case 1:
      return `账户余额 ${(value / 100).toFixed(2)}`;
    case 2:
      return `订阅时长 ${value} 天`;
    case 3:
      return `套餐流量 ${value} GB`;
    case 4:
      return '流量已重置';
    case 5:
      return `订阅套餐 ${value} 天`;
    default:
      return '未知类型';
  }
}

function formatCentsPlain(cents: number) {
  return (parseInt(String(cents), 10) / 100).toFixed(2);
}
