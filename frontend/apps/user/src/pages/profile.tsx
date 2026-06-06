import { useEffect, useRef, useState } from 'react';
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
import { QuestionCircleIcon } from '@/components/ant-icon';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';

export default function ProfilePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const info = useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  // The original /profile never dispatches user/getSubscribe on mount; it only reads
  // whatever subscribe data already sits in the dva store (populated by dashboard/node)
  // and re-fetches solely after unbinding Telegram. Mirror that: read the cached query
  // without an eager mount fetch, while subscribeQuery.refetch() in onUnbindTelegram works.
  const subscribeQuery = useSubscribe({ enabled: false });
  const subscribe = subscribeQuery.data;
  const updateProfile = useUpdateProfileMutation();
  const changePassword = useChangePasswordMutation();
  const redeem = useRedeemGiftCardMutation();
  const resetSub = useResetSubscribeMutation();
  const unbindTelegram = useUnbindTelegramMutation();

  const giftCardRef = useRef<HTMLInputElement>(null);
  const oldPasswordRef = useRef<HTMLInputElement>(null);
  const newPasswordRef = useRef<HTMLInputElement>(null);
  const confirmPasswordRef = useRef<HTMLInputElement>(null);
  const [depositOpen, setDepositOpen] = useState(false);
  const [telegramOpen, setTelegramOpen] = useState(false);
  const [updatingPref, setUpdatingPref] = useState({
    auto_renewal: false,
    remind_expire: false,
    remind_traffic: false,
  });
  const botInfo = useTelegramBotInfo(telegramOpen);
  const depositBodyRef = useRef<HTMLDivElement>(null);
  const depositInputRef = useRef<HTMLInputElement>(null);
  const depositAmountRef = useRef<number | undefined>(undefined);

  // antd Modal.confirm defaults autoFocusButton:"ok" — focus the OK button when the deposit
  // modal opens (this parent effect runs after DialogContent focuses the dialog wrap).
  useEffect(() => {
    if (depositOpen) {
      depositBodyRef.current?.querySelector<HTMLButtonElement>('.ant-btn-primary')?.focus();
    }
  }, [depositOpen]);

  const data = info.data;
  const currency = comm?.currency;
  const depositPlaceholder = t(`请输入充值金额${currency}`);

  const togglePref = async (
    key: 'auto_renewal' | 'remind_expire' | 'remind_traffic',
    value: 0 | 1,
  ) => {
    let succeeded = false;
    setUpdatingPref((current) => ({ ...current, [key]: true }));
    try {
      await updateProfile.mutateAsync({ [key]: value } as Parameters<
        typeof updateProfile.mutateAsync
      >[0]);
      succeeded = true;
    } catch {
    } finally {
      setUpdatingPref((current) => ({ ...current, [key]: false }));
    }
    if (succeeded) void info.refetch();
  };

  const onChangePwd = async () => {
    const oldPassword = oldPasswordRef.current!.value;
    const newPassword = newPasswordRef.current!.value;
    const confirmPassword = confirmPasswordRef.current!.value;
    if (newPassword !== confirmPassword) {
      toast.error(t('profile.password_mismatch'));
      return;
    }
    try {
      await changePassword.mutateAsync({ oldPassword, newPassword });
      toast.success('修改成功，请重新登陆');
      navigate('/login');
    } catch {}
  };

  const onRedeem = async () => {
    const giftcard = giftCardRef.current!.value;
    if (giftcard.length === 0) {
      toast.error(t('profile.redeem_placeholder'));
      return;
    }
    try {
      const result = await redeem.mutateAsync(giftcard);
      void info.refetch();
      toast.success(`兑换成功: ${redeemGiftcardText(result.type, result.value)}`);
    } catch {}
  };

  const onReset = () => {
    void legacyConfirm({
      title: t('profile.reset_subscribe_confirm'),
      content: t('profile.reset_subscribe_tip'),
      okText: t('profile.confirm'),
      cancelText: t('common.cancel'),
      onOk: () => {
        void resetSub
          .mutateAsync()
          .then(() => {
            toast.success(t('profile.reset_success'));
          })
          .catch(() => {});
      },
    });
  };

  const onUnbindTelegram = () => {
    void legacyConfirm({
      title: t('profile.telegram_unbind_confirm'),
      content: t('profile.telegram_unbind_tip'),
      okText: t('profile.confirm'),
      cancelText: t('common.cancel'),
      onOk: () => {
        void unbindTelegram
          .mutateAsync()
          .then(() => {
            toast.success(t('profile.reset_success'));
            void info.refetch();
            void subscribeQuery.refetch();
          })
          .catch(() => {});
      },
    });
  };

  const onDeposit = () => {
    // The original stores the last typed amount on the page instance and never
    // resets that field; Modal.confirm destroys only the input DOM. Keep that
    // small quirk: a reopened empty modal still submits the previous typed value.
    const depositAmountValue = depositAmountRef.current;
    void user
      .saveOrder(apiClient, {
        plan_id: 0,
        period: 'deposit',
        deposit_amount: depositAmountValue,
      })
      .then((tradeNo) => navigate(`/order/${tradeNo}`))
      .catch(() => {});
    setDepositOpen(false);
    if (depositInputRef.current) depositInputRef.current.value = '';
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
                    loading={updatingPref.auto_renewal}
                    checked={data?.auto_renewal}
                    onChange={(checked) => void togglePref('auto_renewal', checked ? 1 : 0)}
                  />
                </span>
                <div className="pt-3">
                  <AntBtn
                    type="button"
                    className="ant-btn ant-btn-primary"
                    onClick={() => {
                      if (depositInputRef.current) depositInputRef.current.value = '';
                      setDepositOpen(true);
                    }}
                  >
                    {t('profile.recharge')}
                  </AntBtn>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <LegacyBlock title={t('profile.redeem_giftcard')} withOptions={false}>
        <div className="row push">
          <div className="col-lg-8 col-xl-5">
            <div className="form-group">
              <input
                className="form-control"
                placeholder={t('profile.redeem_placeholder')}
                autoComplete="one-time-code"
                ref={giftCardRef}
              />
            </div>
            <AntBtn
              type="button"
              className={`ant-btn ant-btn-primary${redeem.isPending ? ' ant-btn-loading' : ''}`}
              onClick={() => {
                if (!redeem.isPending) void onRedeem();
              }}
            >
              {redeem.isPending && <LegacyLoadingIcon />}
              {t('profile.redeem_submit')}
            </AntBtn>
          </div>
        </div>
      </LegacyBlock>

      <LegacyBlock title={t('profile.change_password')}>
        <div className="row push">
          <div className="col-lg-8 col-xl-5">
            <div className="form-group">
              <label>{t('profile.old_password')}</label>
              <input
                ref={(node) =>
                  setLegacyProfilePasswordInputAttributes(
                    node,
                    oldPasswordRef,
                    t('profile.old_password_placeholder'),
                  )
                }
              />
            </div>
            <div className="form-group">
              <label>{t('profile.new_password')}</label>
              <input
                ref={(node) =>
                  setLegacyProfilePasswordInputAttributes(
                    node,
                    newPasswordRef,
                    t('profile.new_password_placeholder'),
                  )
                }
              />
            </div>
            <div className="form-group">
              <label>{t('profile.new_password')}</label>
              <input
                ref={(node) =>
                  setLegacyProfilePasswordInputAttributes(
                    node,
                    confirmPasswordRef,
                    t('profile.new_password_placeholder'),
                  )
                }
              />
            </div>
            <AntBtn
              type="button"
              className={`ant-btn ant-btn-primary${changePassword.isPending ? ' ant-btn-loading' : ''}`}
              onClick={() => {
                if (!changePassword.isPending) void onChangePwd();
              }}
            >
              {changePassword.isPending && <LegacyLoadingIcon />}
              {t('profile.save')}
            </AntBtn>
          </div>
        </div>
      </LegacyBlock>

      <LegacyBlock title={t('profile.notifications')} withOptions={false}>
        <div className="row">
          <div className="col-lg-8 col-xl-5">
            <div className="form-group">
              <label>{t('profile.remind_expire')}</label>
              <div>
                <LegacySwitch
                  loading={updatingPref.remind_expire}
                  checked={data?.remind_expire}
                  onChange={(checked) => void togglePref('remind_expire', checked ? 1 : 0)}
                />
              </div>
            </div>
            <div className="form-group">
              <label>{t('profile.remind_traffic')}</label>
              <div>
                <LegacySwitch
                  loading={updatingPref.remind_traffic}
                  checked={data?.remind_traffic}
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
                <div className="block-options">{t(`Telegram ID: ${String(data.telegram_id)}`)}</div>
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

          {/* Original class string has a trailing space: "block block-rounded " (umi.js). */}
          <div className="block block-rounded ">
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

      <Dialog
        open={depositOpen}
        onOpenChange={(open) => {
          setDepositOpen(open);
          if (!open && depositInputRef.current) depositInputRef.current.value = '';
        }}
      >
        <DialogContent
          closable={false}
          footer={null}
          width={416}
          maskClosable={false}
          className="ant-modal-confirm ant-modal-confirm-confirm"
        >
          <div className="ant-modal-confirm-body-wrapper" ref={depositBodyRef}>
            <div className="ant-modal-confirm-body">
              <QuestionCircleIcon />
              <span className="ant-modal-confirm-title">
                <input
                  className="form-control"
                  autoComplete="one-time-code"
                  placeholder={depositPlaceholder}
                  ref={depositInputRef}
                  onChange={(event) => {
                    depositAmountRef.current = Number(event.target.value) * 100;
                  }}
                />
              </span>
              {/* antd Modal.confirm always renders an (empty) content div after the
                  title; the deposit modal passes no content, so it stays empty. */}
              <div className="ant-modal-confirm-content" />
            </div>
            <div className="ant-modal-confirm-btns">
              <AntBtn
                type="button"
                className="ant-btn"
                onClick={() => {
                  setDepositOpen(false);
                  if (depositInputRef.current) depositInputRef.current.value = '';
                }}
              >
                {t('common.cancel')}
              </AntBtn>
              <AntBtn type="button" className="ant-btn ant-btn-primary" onClick={() => onDeposit()}>
                {t('profile.confirm')}
              </AntBtn>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={telegramOpen} onOpenChange={setTelegramOpen}>
        <DialogContent
          title={t('profile.telegram_bind')}
          okText={t('profile.i_know')}
          cancelText={t('common.cancel')}
          cancelButtonProps={{ hidden: true }}
          onOk={() => setTelegramOpen(false)}
        >
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
                  /bind {subscribe?.subscribe_url}
                </code>
              </div>
            </>
          ) : (
            <LegacyLoadingIcon style={{ fontSize: 16 }} />
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}

function LegacyBlock({
  title,
  withOptions = true,
  children,
}: {
  title: ReactNode;
  withOptions?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="row mb-3 mb-md-0">
      <div className="col-md-12">
        {/* Original class string has a trailing space: "block block-rounded " (umi.js). */}
        <div className="block block-rounded ">
          <div className="block-header block-header-default">
            <h3 className="block-title">{title}</h3>
            {withOptions ? <div className="block-options" /> : null}
          </div>
          <div className="block-content">{children}</div>
        </div>
      </div>
    </div>
  );
}

// Mirrors antd's Wave with insertExtraNode:true (used by Switch): on click it
// appends an .ant-click-animating-node child and flags ant-click-animating, so
// the CSS ripple plays via the extra node rather than the ::after pseudo. The
// shadow colour stays on --antd-wave-shadow-color, which the loaded legacy theme
// sets exactly like the original theme CSS.
function triggerSwitchWave(node: HTMLElement) {
  const existing = node.querySelector('.ant-click-animating-node');
  if (existing) existing.remove();
  const wave = document.createElement('div');
  wave.className = 'ant-click-animating-node';
  node.setAttribute('ant-click-animating', 'true');
  node.appendChild(wave);
  const onEnd = (event: AnimationEvent) => {
    if (event.animationName !== 'fadeEffect') return;
    node.setAttribute('ant-click-animating', 'false');
    if (node.contains(wave)) node.removeChild(wave);
    node.removeEventListener('animationend', onEnd);
  };
  node.addEventListener('animationend', onEnd);
}

function setLegacyProfilePasswordInputAttributes(
  node: HTMLInputElement | null,
  ref: { current: HTMLInputElement | null },
  placeholder: string,
) {
  ref.current = node;
  if (!node) return;
  node.removeAttribute('class');
  node.removeAttribute('placeholder');
  node.removeAttribute('type');
  node.setAttribute('type', 'password');
  node.setAttribute('class', 'form-control');
  node.setAttribute('placeholder', placeholder);
}

function LegacySwitch({
  checked,
  loading,
  onChange,
}: {
  checked?: unknown;
  loading?: boolean;
  onChange: (checked: boolean) => void;
}) {
  const normalizedChecked = !!checked;
  const className = [
    loading && 'ant-switch-loading',
    'ant-switch',
    normalizedChecked && 'ant-switch-checked',
    loading && 'ant-switch-disabled',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <button
      type="button"
      role="switch"
      aria-checked={normalizedChecked}
      className={className}
      disabled={loading}
      onKeyDown={(event) => {
        // rc-switch handleKeyDown: ArrowLeft (37) → off, ArrowRight (39) → on.
        if (event.keyCode === 37) onChange(false);
        else if (event.keyCode === 39) onChange(true);
      }}
      onMouseUp={(event) => event.currentTarget.blur()}
      onClick={(event) => {
        triggerSwitchWave(event.currentTarget);
        onChange(!normalizedChecked);
      }}
    >
      {loading ? <LegacyLoadingIcon className="ant-switch-loading-icon" /> : null}
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
  return (parseInt(String(cents)) / 100).toFixed(2);
}
