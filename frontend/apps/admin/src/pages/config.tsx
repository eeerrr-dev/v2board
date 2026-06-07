import { useEffect, useRef, useState, type ReactNode } from 'react';
import { App } from 'antd';
import { useLocation } from 'react-router-dom';
import type { AdminConfig, AdminConfigFlat, AdminConfigGroups, Plan } from '@v2board/types';
import type { AdminThemeField, AdminThemeInfo } from '@v2board/api-client';
import { LegacyButton } from '@/components/legacy-button';
import {
  LegacyInput as LegacyAntInput,
  LegacyInputGroup,
  LegacyTextArea as LegacyAntTextArea,
} from '@/components/legacy-input';
import { LegacyLoadingIcon } from '@/components/legacy-ant-icon';
import {
  LegacySelect,
  type LegacySelectOption,
  type LegacySelectValue,
} from '@/components/legacy-select';
import { LegacyModal } from '@/components/legacy-modal';
import { LegacySwitch } from '@/components/legacy-switch';
import { LegacyTabs } from '@/components/legacy-tabs';
import {
  useAdminPlans,
  useConfig,
  useEmailTemplates,
  useSaveConfigMutation,
  useSaveThemeConfigMutation,
  useSetTelegramWebhookMutation,
  useTestSendMailMutation,
  useThemeConfigMutation,
  useThemeTemplates,
  useThemes,
} from '@/lib/queries';
import { i18nGet } from '@/lib/errors';

const THEME_BACKGROUND =
  'https://images.unsplash.com/photo-1567095761054-7a02e69e5c43?ixlib=rb-1.2.1&ixid=MnwxMjA3fDB8MHxwaG90by1wYWdlfHx8fGVufDB8fHx8&auto=format&fit=crop&w=1374&q=80';

type ConfigGroupKey = keyof AdminConfigGroups;
type ConfigState = Partial<Record<ConfigGroupKey, Record<string, unknown>>> & Partial<AdminConfig>;

export default function ConfigPage() {
  const location = useLocation();
  if (location.pathname === '/config/theme') return <ThemeConfigPage />;
  return <SystemConfigPage />;
}

function ThemeConfigPage() {
  const { message } = App.useApp();
  const themes = useThemes();
  const saveConfig = useSaveConfigMutation();
  const themeItems = themes.data?.themes ?? {};
  const active = themes.data?.active;
  const themeError = themes.isError;
  const loading = !themeError && Object.keys(themeItems).length <= 0;

  const activateTheme = (name: string) => {
    saveConfig
      .mutateAsync({ frontend_theme: name })
      .then(() => {
        void themes.refetch();
      })
      .catch((error) => showError(message, error));
  };

  if (loading) {
    return (
      <div className="content content-full text-center pt-5">
        <div className="spinner-grow text-primary" role="status">
          <span className="sr-only">Loading...</span>
        </div>
      </div>
    );
  }

  if (themeError) {
    return (
      <div className="block block-rounded">
        <div className="block-content text-center py-5">
          <h3 className="font-w400 text-danger mb-2">页面加载失败</h3>
          <p className="text-muted mb-4">主题配置加载失败，请刷新页面后重试。</p>
          <button type="button" className="btn btn-primary" onClick={() => void themes.refetch()}>
            重试
          </button>
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="row">
        <div className="col-lg-12">
          <div className="alert alert-warning mb-0 mb-md-4" role="alert">
            <p className="mb-0">
              如果你采用前后分离的方式部署V2board，那么主题配置将不会生效。了解
              <b>
                <a href="https://docs.v2board.com/use/advanced.html#%E5%89%8D%E7%AB%AF%E5%88%86%E7%A6%BB">
                  前后分离
                </a>
              </b>
            </p>
          </div>
        </div>
      </div>
      {Object.entries(themeItems).map(([key, theme]) => {
        return (
          <div
            className="block block-transparent bg-image mb-0 mb-md-3 bg-primary"
            style={{ backgroundImage: `url(${THEME_BACKGROUND})` }}
          >
            <div className="block-content block-content-full bg-gd-white-op-l">
              <div className="d-md-flex justify-content-md-between align-items-md-center">
                <div className="p-2 py-4">
                  <h3 className="font-size-h4 font-w400 text-black mb-1">{theme.name}</h3>
                  <p className="text-black-75 mb-0">{theme.description}</p>
                </div>
                <div className="p-2 py-4">
                  <button
                    type="button"
                    className="btn btn-sm rounded-pill btn-outline-light px-3 mr-2"
                    onClick={() => activateTheme(key)}
                    disabled={active === key}
                  >
                    {active === key ? '当前主题' : '激活主题'}
                  </button>
                  <ThemeSettingsButton
                    themeKey={key}
                    theme={theme}
                    onSaved={() => themes.refetch()}
                  />
                </div>
              </div>
            </div>
          </div>
        );
      })}
    </>
  );
}

function ThemeSettingsButton({
  themeKey,
  theme,
  onSaved,
}: {
  themeKey: string;
  theme: AdminThemeInfo;
  onSaved: () => void | Promise<unknown>;
}) {
  const { message } = App.useApp();
  const getConfig = useThemeConfigMutation();
  const saveConfig = useSaveThemeConfigMutation();
  const [visible, setVisible] = useState(false);
  const [params, setParams] = useState<Record<string, unknown>>({});

  const show = () => {
    setVisible(true);
    getConfig
      .mutateAsync(themeKey)
      .then((data) => setParams(data))
      .catch((error) => showError(message, error));
  };

  const hide = () => {
    setVisible(false);
    setParams({});
  };

  const save = async () => {
    try {
      await saveConfig.mutateAsync({ name: themeKey, config: encodeLegacyThemeConfig(params) });
      void onSaved();
      message.success('保存成功');
    } catch (error) {
      showError(message, error);
    }
  };

  return (
    <>
      <button
        type="button"
        className="btn btn-sm rounded-pill btn-outline-light px-3"
        onClick={show}
      >
        主题设置
      </button>
      <LegacyModal
        title={`配置${theme.name}主题`}
        visible={visible}
        onCancel={hide}
        okButtonProps={{ loading: saveConfig.isPending }}
        onOk={save}
      >
        {(theme.configs ?? []).map((field) => (
          <div className="form-group">
            <label>{field.label}</label>
            <ThemeField
              field={field}
              value={params[field.field_name]}
              onChange={(value) => setParams((state) => ({ ...state, [field.field_name]: value }))}
            />
          </div>
        ))}
      </LegacyModal>
    </>
  );
}

function ThemeField({
  field,
  value,
  onChange,
}: {
  field: AdminThemeField;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  if (field.field_type === 'select') {
    const options = field.select_options as Record<string, string>;
    const selectOptions: LegacySelectOption[] = Object.keys(options).map((key) => ({
      value: key,
      label: options[key] ?? '',
    }));
    return (
      <div>
        <LegacySelect
          style={{ width: '100%' }}
          placeholder={field.placeholder}
          value={value as LegacySelectValue | undefined}
          options={selectOptions}
          onChange={(next) => onChange(next)}
        />
      </div>
    );
  }
  if (field.field_type === 'textarea') {
    return (
      <LegacyAntTextArea
        rows={5}
        className="ant-input"
        placeholder={field.placeholder}
        value={toText(value)}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }
  if (field.field_type === 'input') {
    return (
      <LegacyAntInput
        className="ant-input"
        placeholder={field.placeholder}
        value={toText(value)}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }
  return undefined;
}

function encodeLegacyThemeConfig(params: Record<string, unknown>) {
  const json = JSON.stringify(params);
  if (typeof window === 'undefined') return Buffer.from(json).toString('base64');
  return window.btoa(unescape(encodeURIComponent(json)));
}

function SystemConfigPage() {
  const { message, notification } = App.useApp();
  const config = useConfig();
  const plans = useAdminPlans();
  const emailTemplates = useEmailTemplates();
  useThemeTemplates();
  const save = useSaveConfigMutation();
  const webhook = useSetTelegramWebhookMutation();
  const testMail = useTestSendMailMutation();
  const [activeTab, setActiveTab] = useState<ConfigGroupKey>('site');
  const [state, setState] = useState<ConfigState>(() => (config.data ?? {}) as ConfigState);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (config.data) setState(config.data as ConfigState);
  }, [config.data]);

  useEffect(
    () => () => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
    },
    [],
  );

  const group = (key: ConfigGroupKey) => (state[key] ?? {}) as Record<string, unknown>;
  const value = (key: ConfigGroupKey, field: string) => group(key)[field];

  const scheduleSave = (parentKey: ConfigGroupKey, nextGroup: Record<string, unknown>) => {
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      saveTimer.current = null;
      save
        .mutateAsync(nextGroup as Partial<AdminConfigFlat>)
        .then(() => {
          message.success('保存成功');
          void config.refetch();
        })
        .catch((error) => showError(message, error));
    }, 1500);
  };

  const setConfigValue = (parentKey: ConfigGroupKey, field: string, nextValue: unknown) => {
    setState((current) => {
      const nextGroup = {
        ...((current[parentKey] as Record<string, unknown> | undefined) ?? {}),
        [field]: nextValue,
      };
      scheduleSave(parentKey, nextGroup);
      return {
        ...current,
        [parentKey]: nextGroup,
      };
    });
  };

  const sendTestMail = () => {
    testMail
      .mutateAsync()
      .then((result) => {
        const log = result.log;
        const failed = Boolean(log?.error);
        const title = failed ? '发送失败' : '发送成功';
        const content = (
          <div>
            {log?.error ? (
              <div>
                <span>失败原因:</span>
                <span>{log.error}</span>
              </div>
            ) : null}
            <div>
              <span>收信地址:</span>
              <span>{log?.email}</span>
            </div>
            <div>
              <span>发信服务器:</span>
              <span>{log?.config!.host}</span>
            </div>
            <div>
              <span>发信端口:</span>
              <span>{log?.config!.port}</span>
            </div>
            <div>
              <span>发信加密方式:</span>
              <span>{log?.config!.encryption}</span>
            </div>
            <div>
              <span>发信用户名:</span>
              <span>{log?.config!.username}</span>
            </div>
          </div>
        );
        const notice = {
          title,
          content,
          message: title,
          description: content,
        };
        notification[failed ? 'error' : 'success'](notice);
        console.log(result);
      })
      .catch((error) => showError(message, error));
  };

  const setWebhook = () => {
    webhook
      .mutateAsync()
      .then(() => message.success('webhook 设置成功'))
      .catch((error) => showError(message, error));
  };

  return (
    <div className={`mb-0 block border-bottom ${config.isFetching ? 'block-mode-loading' : ''}`}>
      <LegacyTabs
        defaultActiveKey={activeTab}
        onChange={(key) => setActiveTab(key as ConfigGroupKey)}
        size="large"
      >
        <LegacyTabs.TabPane tab="站点" key="site">
          <div className="">
            <ConfigItem title="站点名称" description="用于显示需要站点名称的地方。">
              <LegacyInput
                placeholder="请输入站点名称"
                value={value('site', 'app_name')}
                onChange={(next) => setConfigValue('site', 'app_name', next)}
              />
            </ConfigItem>
            <ConfigItem title="站点描述" description="用于显示需要站点描述的地方。">
              <LegacyInput
                placeholder="请输入站点描述"
                value={value('site', 'app_description')}
                onChange={(next) => setConfigValue('site', 'app_description', next)}
              />
            </ConfigItem>
            <ConfigItem
              title="站点网址"
              description="当前网站最新网址，将会在邮件等需要用于网址处体现。"
            >
              <LegacyInput
                placeholder="请输入站点URL，末尾不要/"
                value={value('site', 'app_url')}
                onChange={(next) => setConfigValue('site', 'app_url', next)}
              />
            </ConfigItem>
            <ConfigItem
              title="强制HTTPS"
              description="当站点没有使用HTTPS，CDN或反代开启强制HTTPS时需要开启。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('site', 'force_https'))}
                onChange={(checked) => setConfigValue('site', 'force_https', checked ? 1 : 0)}
              />
            </ConfigItem>
            <ConfigItem title="LOGO" description="用于显示需要LOGO的地方。">
              <LegacyInput
                placeholder="请输入LOGO URL，末尾不要/"
                value={value('site', 'logo')}
                onChange={(next) => setConfigValue('site', 'logo', next)}
              />
            </ConfigItem>
            <ConfigItem
              title="订阅URL"
              description="用于订阅所使用，留空则为站点URL。如需多个订阅URL随机获取请使用逗号进行分割。"
            >
              <LegacyTextarea
                rows={4}
                placeholder="请输入订阅URL，末尾不要/。逗号分割支持多域名"
                value={value('site', 'subscribe_url')}
                onChange={(next) => setConfigValue('site', 'subscribe_url', next)}
              />
            </ConfigItem>
            <ConfigItem
              title="订阅路径"
              description="用于订阅所使用，留空则为/api/v1/client/subscribe。如需更换不同的订阅路径请设置。"
            >
              <LegacyInput
                placeholder="/api/v1/client/subscribe"
                value={value('site', 'subscribe_path')}
                onChange={(next) => setConfigValue('site', 'subscribe_path', next)}
              />
            </ConfigItem>
            <ConfigItem title="用户条款(TOS)URL" description="用于跳转到用户条款(TOS)">
              <LegacyInput
                placeholder="请输入用户条款URL，末尾不要/"
                value={value('site', 'tos_url')}
                onChange={(next) => setConfigValue('site', 'tos_url', next)}
              />
            </ConfigItem>
            <ConfigItem title="停止新用户注册" description="开启后任何人都将无法进行注册。">
              <LegacySwitch
                checked={isLegacyChecked(value('site', 'stop_register'))}
                onChange={(checked) => setConfigValue('site', 'stop_register', checked ? 1 : 0)}
              />
            </ConfigItem>
            <ConfigItem
              title="注册试用"
              description="选择需要试用的订阅，如果没有选项请先前往订阅管理添加。"
            >
              <select
                className="form-control"
                value={legacySelectValue(value('site', 'try_out_plan_id'))}
                placeholder="请选择试用订阅"
                onChange={(event) => setConfigValue('site', 'try_out_plan_id', event.target.value)}
              >
                <option value={0}>关闭</option>
                {(plans.data ?? []).map((plan: Plan) => (
                  <option key={Math.random()} value={plan.id}>
                    {plan.name}
                  </option>
                ))}
              </select>
            </ConfigItem>
            {value('site', 'try_out_plan_id') === 0 ? null : (
              <ConfigItem isChildren title="试用时间(小时)">
                <LegacyInput
                  placeholder="请输入"
                  value={value('site', 'try_out_hour')}
                  onChange={(next) => setConfigValue('site', 'try_out_hour', next)}
                />
              </ConfigItem>
            )}
            <ConfigItem
              title="货币单位"
              description="仅用于展示使用，更改后系统中所有的货币单位都将发生变更。"
            >
              <LegacyInput
                placeholder="CNY"
                value={value('site', 'currency')}
                onChange={(next) => setConfigValue('site', 'currency', next)}
              />
            </ConfigItem>
            <ConfigItem
              title="货币符号"
              description="仅用于展示使用，更改后系统中所有的货币单位都将发生变更。"
            >
              <LegacyInput
                placeholder="¥"
                value={value('site', 'currency_symbol')}
                onChange={(next) => setConfigValue('site', 'currency_symbol', next)}
              />
            </ConfigItem>
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="安全" key="safe">
          <div className="">
            <ConfigItem title="邮箱验证" description="开启后将会强制要求用户进行邮箱验证。">
              <LegacySwitch
                checked={isLegacyChecked(value('safe', 'email_verify'))}
                onChange={(checked) => setConfigValue('safe', 'email_verify', checked ? 1 : 0)}
              />
            </ConfigItem>
            <ConfigItem title="禁止使用Gmail多别名" description="开启后Gmail多别名将无法注册。">
              <LegacySwitch
                checked={isLegacyChecked(value('safe', 'email_gmail_limit_enable'))}
                onChange={(checked) =>
                  setConfigValue('safe', 'email_gmail_limit_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="安全模式"
              description="开启后除了站点URL以外的绑定本站点的域名访问都将会被403。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('safe', 'safe_mode_enable'))}
                onChange={(checked) => setConfigValue('safe', 'safe_mode_enable', checked ? 1 : 0)}
              />
            </ConfigItem>
            <ConfigItem title="后台路径" description="后台管理路径，修改后将会改变原有的admin路径">
              <LegacyInput
                placeholder="admin"
                value={value('safe', 'secure_path')}
                onChange={(next) => setConfigValue('safe', 'secure_path', next)}
              />
            </ConfigItem>
            <ConfigItem
              title="邮箱后缀白名单"
              description="开启后在名单中的邮箱后缀才允许进行注册。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('safe', 'email_whitelist_enable'))}
                onChange={(checked) =>
                  setConfigValue('safe', 'email_whitelist_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            {value('safe', 'email_whitelist_enable') ? (
              <ConfigItem
                isChildren
                title="白名单后缀"
                description="请使用逗号进行分割，如：qq.com,gmail.com。"
              >
                <LegacyTextarea
                  rows={4}
                  placeholder="请输入后缀域名，逗号分割 如：qq.com,gmail.com"
                  value={value('safe', 'email_whitelist_suffix')}
                  onChange={(next) =>
                    setConfigValue('safe', 'email_whitelist_suffix', splitComma(next))
                  }
                />
              </ConfigItem>
            ) : null}
            <ConfigItem title="防机器人" description="开启后将会使用Google reCAPTCHA防止机器人。">
              <LegacySwitch
                checked={isLegacyChecked(value('safe', 'recaptcha_enable'))}
                onChange={(checked) => setConfigValue('safe', 'recaptcha_enable', checked ? 1 : 0)}
              />
            </ConfigItem>
            {value('safe', 'recaptcha_enable') ? (
              <>
                <ConfigItem isChildren title="密钥" description="在Google reCAPTCHA申请的密钥。">
                  <LegacyInput
                    placeholder="请输入"
                    value={value('safe', 'recaptcha_key')}
                    onChange={(next) => setConfigValue('safe', 'recaptcha_key', next)}
                  />
                </ConfigItem>
                <ConfigItem
                  isChildren
                  title="网站密钥"
                  description="在Google reCAPTCH申请的网站密钥。"
                >
                  <LegacyInput
                    placeholder="请输入"
                    value={value('safe', 'recaptcha_site_key')}
                    onChange={(next) => setConfigValue('safe', 'recaptcha_site_key', next)}
                  />
                </ConfigItem>
              </>
            ) : null}
            <ConfigItem
              title="IP注册限制"
              description="开启后如果IP注册账户达到规则要求将会被限制注册，请注意IP判断可能因为CDN或前置代理导致问题。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('safe', 'register_limit_by_ip_enable'))}
                onChange={(checked) =>
                  setConfigValue('safe', 'register_limit_by_ip_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            {value('safe', 'register_limit_by_ip_enable') ? (
              <>
                <ConfigItem isChildren title="次数" description="达到注册次数后开启惩罚。">
                  <LegacyInput
                    placeholder="请输入"
                    value={value('safe', 'register_limit_count')}
                    onChange={(next) => setConfigValue('safe', 'register_limit_count', next)}
                  />
                </ConfigItem>
                <ConfigItem
                  isChildren
                  title="惩罚时间(分钟)"
                  description="需要等待惩罚时间过后才可以再次注册。"
                >
                  <LegacyInput
                    placeholder="请输入"
                    value={value('safe', 'register_limit_expire')}
                    onChange={(next) => setConfigValue('safe', 'register_limit_expire', next)}
                  />
                </ConfigItem>
              </>
            ) : null}
            <ConfigItem
              title="防爆破限制"
              description="开启后如果该账户尝试登陆失败次数过多将会被限制。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('safe', 'password_limit_enable'))}
                onChange={(checked) =>
                  setConfigValue('safe', 'password_limit_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            {value('safe', 'password_limit_enable') ? (
              <>
                <ConfigItem isChildren title="次数" description="达到失败次数后开启惩罚。">
                  <LegacyInput
                    placeholder="请输入"
                    value={value('safe', 'password_limit_count')}
                    onChange={(next) => setConfigValue('safe', 'password_limit_count', next)}
                  />
                </ConfigItem>
                <ConfigItem
                  isChildren
                  title="惩罚时间(分钟)"
                  description="需要等待惩罚时间过后才可以再次登陆。"
                >
                  <LegacyInput
                    placeholder="请输入"
                    value={value('safe', 'password_limit_expire')}
                    onChange={(next) => setConfigValue('safe', 'password_limit_expire', next)}
                  />
                </ConfigItem>
              </>
            ) : null}
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="订阅" key="subscribe">
          <div className="">
            <ConfigItem
              title="允许用户更改订阅"
              description="开启后用户将会可以对订阅计划进行变更。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('subscribe', 'plan_change_enable'))}
                onChange={(checked) =>
                  setConfigValue('subscribe', 'plan_change_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="月流量重置方式"
              description="全局流量重置方式，默认每月1号。可以在订阅管理为订阅单独设置。"
            >
              <select
                className="form-control"
                value={legacySelectValue(value('subscribe', 'reset_traffic_method'))}
                placeholder="请选择订阅重置方式"
                onChange={(event) =>
                  setConfigValue('subscribe', 'reset_traffic_method', event.target.value)
                }
              >
                <option value={0}>每月1号</option>
                <option value={1}>按月重置</option>
                <option value={2}>不重置</option>
                <option value={3}>每年1月1日</option>
                <option value={4}>按年重置</option>
              </select>
            </ConfigItem>
            <ConfigItem
              title="开启折抵方案"
              description="开启后用户更换订阅将会由系统对原有订阅进行折抵，方案参考文档。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('subscribe', 'surplus_enable'))}
                onChange={(checked) =>
                  setConfigValue('subscribe', 'surplus_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="允许提前开启流量周期"
              description="开启后用户流量用尽时可以选择扣除订阅时长为代价重置流量，按月重置时扣除本周期剩余订阅时长，每月1号重置时扣除整月时间30天。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('subscribe', 'allow_new_period'))}
                onChange={(checked) =>
                  setConfigValue('subscribe', 'allow_new_period', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <OrderEventSelect
              title="当订阅新购时触发事件"
              description="新购订阅完成时将触发该任务。"
              value={value('subscribe', 'new_order_event_id')}
              onChange={(next) => setConfigValue('subscribe', 'new_order_event_id', next)}
            />
            <OrderEventSelect
              title="当订阅续费时触发事件"
              description="续费订阅完成时将触发该任务。"
              value={value('subscribe', 'renew_order_event_id')}
              onChange={(next) => setConfigValue('subscribe', 'renew_order_event_id', next)}
            />
            <OrderEventSelect
              title="当订阅变更时触发事件"
              description="变更订阅完成时将触发该任务。"
              value={value('subscribe', 'change_order_event_id')}
              onChange={(next) => setConfigValue('subscribe', 'change_order_event_id', next)}
            />
            <ConfigItem
              title="在订阅中展示订阅信息"
              description="开启后将会在用户订阅节点时输出订阅信息。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('subscribe', 'show_info_to_server_enable'))}
                onChange={(checked) =>
                  setConfigValue('subscribe', 'show_info_to_server_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem title="订阅链接生效模式" description="用户获取订阅链接后的有效期。">
              <select
                className="form-control"
                value={legacySelectValue(value('subscribe', 'show_subscribe_method'))}
                placeholder="请选择"
                onChange={(event) =>
                  setConfigValue('subscribe', 'show_subscribe_method', event.target.value)
                }
              >
                <option value={0}>永久有效</option>
                <option value={1}>一次性有效</option>
                <option value={2}>限时有效</option>
              </select>
            </ConfigItem>
            {value('subscribe', 'show_subscribe_method') == 2 ? (
              <ConfigItem
                isChildren
                title="订阅链接有效时间(分钟)"
                description="订阅链接获取后经过该时间将失效。"
              >
                <LegacyInput
                  placeholder="请输入"
                  value={value('subscribe', 'show_subscribe_expire')}
                  onChange={(next) => setConfigValue('safe', 'show_subscribe_expire', next)}
                />
              </ConfigItem>
            ) : null}
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="充值" key="deposit">
          <div className="">
            <ConfigItem title="充值奖励" description="充值一定金额可以获得的奖励。">
              <LegacyTextarea
                rows={2}
                placeholder={'请输入 充值金额:奖励金额,逗号分割\n如 50:18,100:38, 200:88'}
                value={value('deposit', 'deposit_bounus')}
                onChange={(next) => setConfigValue('deposit', 'deposit_bounus', splitComma(next))}
              />
            </ConfigItem>
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="工单" key="ticket">
          <div className="">
            <ConfigItem title="工单设置" description="请选择工单的状态。">
              <select
                className="form-control"
                value={legacySelectValue(value('ticket', 'ticket_status') || 0)}
                onChange={(event) => setConfigValue('ticket', 'ticket_status', event.target.value)}
              >
                <option value={0}>完全开放工单</option>
                <option value={1}>仅限有付费订单用户</option>
                <option value={2}>完全禁止工单</option>
              </select>
            </ConfigItem>
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="邀请&佣金" key="invite">
          <div className="">
            <ConfigItem title="开启强制邀请" description="开启后只有被邀请的用户才可以进行注册。">
              <LegacySwitch
                checked={isLegacyChecked(value('invite', 'invite_force'))}
                onChange={(checked) => setConfigValue('invite', 'invite_force', checked ? 1 : 0)}
              />
            </ConfigItem>
            <ConfigItem
              title="邀请佣金百分比"
              description="默认全局的佣金分配比例，你可以在用户管理单独配置单个比例。"
            >
              <LegacyInput
                placeholder="请输入"
                value={value('invite', 'invite_commission')}
                onChange={(next) =>
                  setConfigValue('invite', 'invite_commission', parseLegacyInteger(next))
                }
              />
            </ConfigItem>
            <ConfigItem title="用户可创建邀请码上限">
              <LegacyInput
                placeholder="请输入"
                value={value('invite', 'invite_gen_limit')}
                onChange={(next) =>
                  setConfigValue('invite', 'invite_gen_limit', parseLegacyInteger(next))
                }
              />
            </ConfigItem>
            <ConfigItem
              title="邀请码永不失效"
              description="开启后邀请码被使用后将不会失效，否则使用过后即失效。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('invite', 'invite_never_expire'))}
                onChange={(checked) =>
                  setConfigValue('invite', 'invite_never_expire', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="佣金仅首次发放"
              description="开启后被邀请人首次支付时才会产生佣金，可以在用户管理对用户进行单独配置。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('invite', 'commission_first_time_enable'))}
                onChange={(checked) =>
                  setConfigValue('invite', 'commission_first_time_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="佣金自动确认"
              description="开启后佣金将会在订单完成3日后自动进行确认。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('invite', 'commission_auto_check_enable'))}
                onChange={(checked) =>
                  setConfigValue('invite', 'commission_auto_check_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem title="提现单申请门槛(元)" description="小于门槛金额的提现单将不会被提交。">
              <LegacyInput
                placeholder="请输入"
                value={value('invite', 'commission_withdraw_limit')}
                onChange={(next) => setConfigValue('invite', 'commission_withdraw_limit', next)}
              />
            </ConfigItem>
            <ConfigItem title="提现方式" description="可以支持的提现方式。">
              <LegacyTextarea
                rows={4}
                placeholder="请输入后缀域名，逗号分割 如：支付宝,USDT,贝宝"
                value={value('invite', 'commission_withdraw_method')}
                onChange={(next) =>
                  setConfigValue('invite', 'commission_withdraw_method', splitComma(next))
                }
              />
            </ConfigItem>
            <ConfigItem
              title="关闭提现"
              description="关闭后将禁止用户申请提现，且邀请佣金将会直接进入用户余额。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('invite', 'withdraw_close_enable'))}
                onChange={(checked) =>
                  setConfigValue('invite', 'withdraw_close_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="三级分销"
              description="开启后将佣金将按照设置的3成比例进行分成，三成比例合计请不要>100%。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('invite', 'commission_distribution_enable'))}
                onChange={(checked) =>
                  setConfigValue('invite', 'commission_distribution_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            {isLegacyChecked(value('invite', 'commission_distribution_enable')) ? (
              <>
                <ConfigItem isChildren title="一级邀请人比例">
                  <LegacyInput
                    placeholder="请输入比例如：50"
                    value={value('invite', 'commission_distribution_l1')}
                    onChange={(next) =>
                      setConfigValue('invite', 'commission_distribution_l1', next)
                    }
                  />
                </ConfigItem>
                <ConfigItem isChildren title="二级邀请人比例">
                  <LegacyInput
                    placeholder="请输入比例如：30"
                    value={value('invite', 'commission_distribution_l2')}
                    onChange={(next) =>
                      setConfigValue('invite', 'commission_distribution_l2', next)
                    }
                  />
                </ConfigItem>
                <ConfigItem isChildren title="三级邀请人比例">
                  <LegacyInput
                    placeholder="请输入比例如：20"
                    value={value('invite', 'commission_distribution_l3')}
                    onChange={(next) =>
                      setConfigValue('invite', 'commission_distribution_l3', next)
                    }
                  />
                </ConfigItem>
              </>
            ) : null}
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="个性化" key="frontend">
          <div className="block-content">
            <div className="row">
              <div className="col-lg-12">
                <div className="alert alert-warning" role="alert">
                  <p className="mb-0">
                    如果你采用前后分离的方式部署V2board管理端，那么本页配置将不会生效。了解
                    <b>
                      <a href="https://docs.v2board.com/use/advanced.html#%E5%89%8D%E7%AB%AF%E5%88%86%E7%A6%BB">
                        前后分离
                      </a>
                    </b>
                  </p>
                </div>
              </div>
            </div>
            <div className="">
              <ConfigItem title="边栏风格">
                <LegacySwitch
                  checkedChildren="亮"
                  unCheckedChildren="暗"
                  checked={value('frontend', 'frontend_theme_sidebar') === 'light'}
                  onChange={(checked) =>
                    setConfigValue('site', 'frontend_theme_sidebar', checked ? 'light' : 'dark')
                  }
                />
              </ConfigItem>
              <ConfigItem title="头部风格">
                <LegacySwitch
                  checkedChildren="亮"
                  unCheckedChildren="暗"
                  checked={value('frontend', 'frontend_theme_header') === 'light'}
                  onChange={(checked) =>
                    setConfigValue('site', 'frontend_theme_header', checked ? 'light' : 'dark')
                  }
                />
              </ConfigItem>
              <ConfigItem title="主题色">
                <select
                  className="form-control"
                  defaultValue={legacySelectValue(value('frontend', 'frontend_theme_color'))}
                  onChange={(event) =>
                    setConfigValue('frontend', 'frontend_theme_color', event.target.value)
                  }
                >
                  <option value="default">默认</option>
                  <option value="black">黑色</option>
                  <option value="darkblue">暗蓝色</option>
                  <option value="green">奶绿色</option>
                </select>
              </ConfigItem>
              <ConfigItem title="背景" description="将会在后台登录页面进行展示。">
                <LegacyInput
                  placeholder="https://xxxxx.com/wallpaper.png"
                  value={value('frontend', 'frontend_background_url')}
                  onChange={(next) => setConfigValue('frontend', 'frontend_background_url', next)}
                />
              </ConfigItem>
            </div>
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="节点" key="server">
          <div className="">
            <ConfigItem title="节点对接API地址" description="v2node节点一键对接专用地址。">
              <LegacyInput
                placeholder="请输入"
                value={value('server', 'server_api_url')}
                onChange={(next) => setConfigValue('server', 'server_api_url', next)}
              />
            </ConfigItem>
            <ConfigItem
              title="通讯密钥"
              description="V2board与节点通讯的密钥，以便数据不会被他人获取。"
            >
              <LegacyInput
                placeholder="请输入"
                value={value('server', 'server_token')}
                onChange={(next) => setConfigValue('server', 'server_token', next)}
              />
            </ConfigItem>
            <ConfigItem title="节点拉取动作轮询间隔" description="节点从面板获取数据的间隔频率。">
              <LegacyInputGroup
                addonAfter="秒"
                size="large"
                type="number"
                placeholder="请输入"
                defaultValue={toText(value('server', 'server_pull_interval'))}
                onChange={(event) =>
                  setConfigValue('server', 'server_pull_interval', event.target.value)
                }
              />
            </ConfigItem>
            <ConfigItem title="节点推送动作轮询间隔" description="节点推送数据到面板的间隔频率。">
              <LegacyInputGroup
                addonAfter="秒"
                size="large"
                type="number"
                placeholder="请输入"
                defaultValue={toText(value('server', 'server_push_interval'))}
                onChange={(event) =>
                  setConfigValue('server', 'server_push_interval', event.target.value)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="节点用户流量上报最低阈值"
              description="每次推送动作仅累计使用流量高于阈值的用户信息会被上报，未上报流量会累计"
            >
              <LegacyInputGroup
                addonAfter="Kb"
                size="large"
                type="number"
                placeholder="请输入"
                defaultValue={toText(value('server', 'server_node_report_min_traffic'))}
                onChange={(event) =>
                  setConfigValue('server', 'server_node_report_min_traffic', event.target.value)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="节点用户设备数统计最低阈值"
              description="每次推送动作仅上报流量高于阈值的在线设备IP地址会被节点统计"
            >
              <LegacyInputGroup
                addonAfter="Kb"
                size="large"
                type="number"
                placeholder="请输入"
                defaultValue={toText(value('server', 'server_device_online_min_traffic'))}
                onChange={(event) =>
                  setConfigValue('server', 'server_device_online_min_traffic', event.target.value)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="全局设备数限制采用宽松模式"
              description="开启后同一IP地址使用多个节点只统计为一个设备"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('server', 'device_limit_mode'))}
                onChange={(checked) =>
                  setConfigValue('server', 'device_limit_mode', checked ? 1 : 0)
                }
              />
            </ConfigItem>
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="邮件" key="email">
          <div className="block-content">
            <div className="row">
              <div className="col-lg-12">
                <div className="alert alert-warning" role="alert">
                  <p className="mb-0">
                    如果你更改了本页配置，需要对队列服务进行重启。另外本页配置优先级高于.env中邮件配置。
                  </p>
                </div>
              </div>
            </div>
            <div className="">
              <ConfigItem title="SMTP服务器地址" description="由邮件服务商提供的服务地址">
                <LegacyInput
                  placeholder="请输入"
                  value={value('email', 'email_host')}
                  onChange={(next) => setConfigValue('email', 'email_host', next)}
                />
              </ConfigItem>
              <ConfigItem title="SMTP服务端口" description="常见的端口有25, 465, 587">
                <LegacyInput
                  placeholder="请输入"
                  value={value('email', 'email_port')}
                  onChange={(next) => setConfigValue('email', 'email_port', next)}
                />
              </ConfigItem>
              <ConfigItem
                title="SMTP加密方式"
                description="465端口加密方式一般为SSL，587端口加密方式一般为TLS"
              >
                <LegacyInput
                  placeholder="请输入"
                  value={value('email', 'email_encryption')}
                  onChange={(next) => setConfigValue('email', 'email_encryption', next)}
                />
              </ConfigItem>
              <ConfigItem title="SMTP账号" description="由邮件服务商提供的账号">
                <LegacyInput
                  placeholder="请输入"
                  value={value('email', 'email_username')}
                  onChange={(next) => setConfigValue('email', 'email_username', next)}
                />
              </ConfigItem>
              <ConfigItem title="SMTP密码" description="由邮件服务商提供的密码">
                <LegacyInput
                  placeholder="请输入"
                  value={value('email', 'email_password')}
                  onChange={(next) => setConfigValue('email', 'email_password', next)}
                />
              </ConfigItem>
              <ConfigItem title="发件地址" description="由邮件服务商提供的发件地址">
                <LegacyInput
                  placeholder="请输入"
                  value={value('email', 'email_from_address')}
                  onChange={(next) => setConfigValue('email', 'email_from_address', next)}
                />
              </ConfigItem>
              <ConfigItem title="邮件模板" description="你可以在文档查看如何自定义邮件模板">
                <select
                  className="form-control"
                  value={legacySelectValue(value('email', 'email_template'))}
                  onChange={(event) =>
                    setConfigValue('email', 'email_template', event.target.value)
                  }
                >
                  {(emailTemplates.data ?? []).map((template) => (
                    <option key={Math.random()} value={template}>
                      {template}
                    </option>
                  ))}
                </select>
              </ConfigItem>
              <ConfigItem title="发送测试邮件" description="邮件将会发送到当前登陆用户邮箱">
                <LegacyButton
                  className={`ant-btn ant-btn-primary${testMail.isPending ? ' ant-btn-loading' : ''}`}
                  disabled={testMail.isPending}
                  onClick={sendTestMail}
                >
                  {testMail.isPending ? <LegacyLoadingIcon /> : null}
                  发送测试邮件
                </LegacyButton>
              </ConfigItem>
            </div>
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="Telegram" key="telegram">
          <div className="">
            <ConfigItem title="机器人Token" description="请输入由Botfather提供的token。">
              <LegacyInput
                placeholder="0000000000:xxxxxxxxx_xxxxxxxxxxxxxxx"
                value={value('telegram', 'telegram_bot_token')}
                onChange={(next) => setConfigValue('telegram', 'telegram_bot_token', next)}
              />
            </ConfigItem>
            {value('telegram', 'telegram_bot_token') ? (
              <ConfigItem
                title="设置Webhook"
                description="对机器人进行Webhook设置，不设置将无法收到Telegram通知。"
              >
                <LegacyButton
                  className={`ant-btn ant-btn-primary${webhook.isPending ? ' ant-btn-loading' : ''}`}
                  onClick={setWebhook}
                  disabled={webhook.isPending}
                >
                  {webhook.isPending ? <LegacyLoadingIcon /> : null}
                  一键设置
                </LegacyButton>
              </ConfigItem>
            ) : null}
            <ConfigItem
              title="开启机器人通知"
              description="开启后bot将会对绑定了telegram的管理员和用户进行基础通知。"
            >
              <LegacySwitch
                checked={isLegacyChecked(value('telegram', 'telegram_bot_enable'))}
                onChange={(checked) =>
                  setConfigValue('telegram', 'telegram_bot_enable', checked ? 1 : 0)
                }
              />
            </ConfigItem>
            <ConfigItem
              title="群组地址"
              description="填写后将会在用户端展示，或者被用于需要的地方。"
            >
              <LegacyInput
                placeholder="https://t.me/xxxxxx"
                value={value('telegram', 'telegram_discuss_link')}
                onChange={(next) => setConfigValue('telegram', 'telegram_discuss_link', next)}
              />
            </ConfigItem>
          </div>
        </LegacyTabs.TabPane>

        <LegacyTabs.TabPane tab="APP" key="app">
          <div className="block-content">
            <div className="row">
              <div className="col-lg-12">
                <div className="alert alert-warning" role="alert">
                  <p className="mb-0">用于自有客户端(APP)的版本管理及更新</p>
                </div>
              </div>
            </div>
            <div className="">
              <ConfigItem title="Windows" description="Windows端版本号及下载地址">
                <LegacyInput
                  placeholder="1.0.0"
                  value={value('app', 'windows_version')}
                  onChange={(next) => setConfigValue('app', 'windows_version', next)}
                />
                <LegacyInput
                  className="form-control mt-1"
                  placeholder="https://xxxx.com/xxx.exe"
                  value={value('app', 'windows_download_url')}
                  onChange={(next) => setConfigValue('app', 'windows_download_url', next)}
                />
              </ConfigItem>
              <ConfigItem title="macOS" description="macOS端版本号及下载地址">
                <LegacyInput
                  placeholder="1.0.0"
                  value={value('app', 'macos_version')}
                  onChange={(next) => setConfigValue('app', 'macos_version', next)}
                />
                <LegacyInput
                  className="form-control mt-1"
                  placeholder="https://xxxx.com/xxx.dmg"
                  value={value('app', 'macos_download_url')}
                  onChange={(next) => setConfigValue('app', 'macos_download_url', next)}
                />
              </ConfigItem>
              <ConfigItem title="Android" description="Android端版本号及下载地址">
                <LegacyInput
                  placeholder="1.0.0"
                  value={value('app', 'android_version')}
                  onChange={(next) => setConfigValue('app', 'android_version', next)}
                />
                <LegacyInput
                  className="form-control mt-1"
                  placeholder="https://xxxx.com/xxx.apk"
                  value={value('app', 'android_download_url')}
                  onChange={(next) => setConfigValue('app', 'android_download_url', next)}
                />
              </ConfigItem>
            </div>
          </div>
        </LegacyTabs.TabPane>
      </LegacyTabs>
    </div>
  );
}

function ConfigItem({
  title,
  description,
  isChildren,
  children,
}: {
  title: string;
  description?: string;
  isChildren?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={`row ${isChildren ? 'v2board-config-children' : ''}`}
      style={{ padding: '20px', borderBottom: '1px solid #eee' }}
    >
      <div className="col-lg-6">
        <div style={{ fontWeight: 'bold', marginBottom: 5 }}>{title}</div>
        <div style={{ fontSize: 12, marginBottom: 5, color: '#666' }}>{description}</div>
      </div>
      <div className="col-lg-6 text-right">{children}</div>
    </div>
  );
}

function LegacyInput({
  value,
  placeholder,
  className = 'form-control',
  onChange,
}: {
  value: unknown;
  placeholder?: string;
  className?: string;
  onChange: (value: string) => void;
}) {
  return (
    <input
      type="text"
      className={className}
      placeholder={placeholder}
      defaultValue={toText(value)}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

function LegacyTextarea({
  value,
  placeholder,
  rows,
  onChange,
}: {
  value: unknown;
  placeholder?: string;
  rows: number;
  onChange: (value: string) => void;
}) {
  return (
    <textarea
      rows={rows}
      {...{ type: 'text' }}
      className="form-control"
      placeholder={placeholder}
      defaultValue={toText(value)}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

function OrderEventSelect({
  title,
  description,
  value,
  onChange,
}: {
  title: string;
  description: string;
  value: unknown;
  onChange: (value: string) => void;
}) {
  return (
    <ConfigItem title={title} description={description}>
      <select
        className="form-control"
        value={legacySelectValue(value)}
        placeholder="请选择事件"
        onChange={(event) => onChange(event.target.value)}
      >
        <option value={0}>不执行任何动作</option>
        <option value={1}>重置用户流量</option>
      </select>
    </ConfigItem>
  );
}

function toText(value: unknown) {
  if (Array.isArray(value)) return value.join(',');
  return value == null ? '' : String(value);
}

function legacySelectValue(value: unknown) {
  return value as string | number | readonly string[] | undefined;
}

function splitComma(value: string) {
  return value.split(',');
}

export function parseLegacyInteger(value: string) {
  return parseInt(value);
}

export function isLegacyChecked(value: unknown) {
  return Boolean(parseInt(toText(value)));
}

function showError(message: ReturnType<typeof App.useApp>['message'], error: unknown) {
  if (error instanceof Error) message.error(i18nGet(error.message));
}
