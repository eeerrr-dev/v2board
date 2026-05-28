import { useEffect } from 'react';
import { App, Button, Card, Form, Input, InputNumber, Select, Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import type { AdminConfig } from '@v2board/types';
import { useConfig, useSaveConfigMutation } from '@/lib/queries';
import { i18nGet } from '@/lib/errors';

export default function ConfigPage() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const config = useConfig();
  const save = useSaveConfigMutation();
  const [form] = Form.useForm<AdminConfig>();

  useEffect(() => {
    if (config.data) form.setFieldsValue(config.data);
  }, [config.data, form]);

  const onFinish = async (values: Partial<AdminConfig>) => {
    try {
      await save.mutateAsync(values);
      message.success(t('common.success'));
    } catch (e) {
      if (e instanceof Error) message.error(i18nGet(e.message));
    }
  };

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.config')}</Typography.Title>
      <Form layout="vertical" form={form} onFinish={onFinish}>
        <Card title={t('admin.config.site')}>
          <div className="grid grid-cols-2 gap-x-4">
            <Form.Item name="app_name" label={t('admin.config.app_name')}>
              <Input />
            </Form.Item>
            <Form.Item name="app_url" label={t('admin.config.app_url')}>
              <Input />
            </Form.Item>
            <Form.Item name="app_description" label={t('admin.config.app_description')}>
              <Input />
            </Form.Item>
            <Form.Item name="logo" label="Logo URL">
              <Input />
            </Form.Item>
            <Form.Item name="subscribe_url" label={t('admin.config.subscribe_url')}>
              <Input />
            </Form.Item>
            <Form.Item name="subscribe_path" label={t('admin.config.subscribe_path')}>
              <Input />
            </Form.Item>
            <Form.Item name="secure_path" label={t('admin.config.secure_path')}>
              <Input />
            </Form.Item>
            <Form.Item name="force_https" label={t('admin.config.force_https')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="currency" label={t('admin.config.currency')}>
              <Input />
            </Form.Item>
            <Form.Item name="currency_symbol" label={t('admin.config.currency_symbol')}>
              <Input />
            </Form.Item>
          </div>
        </Card>

        <Card title={t('admin.config.invite')} className="mt-4">
          <div className="grid grid-cols-2 gap-x-4">
            <Form.Item name="invite_force" label={t('admin.config.invite_force')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="invite_commission" label={t('admin.config.invite_commission')}>
              <InputNumber min={0} max={100} />
            </Form.Item>
            <Form.Item name="invite_gen_limit" label={t('admin.config.invite_gen_limit')}>
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item name="invite_never_expire" label={t('admin.config.invite_never_expire')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item
              name="commission_distribution_enable"
              label={t('admin.config.commission_distribution')}
            >
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item
              name="commission_distribution_l1"
              label={t('admin.config.commission_distribution_l1')}
            >
              <InputNumber min={0} max={100} />
            </Form.Item>
            <Form.Item
              name="commission_distribution_l2"
              label={t('admin.config.commission_distribution_l2')}
            >
              <InputNumber min={0} max={100} />
            </Form.Item>
            <Form.Item
              name="commission_distribution_l3"
              label={t('admin.config.commission_distribution_l3')}
            >
              <InputNumber min={0} max={100} />
            </Form.Item>
            <Form.Item
              name="commission_withdraw_limit"
              label={t('admin.config.commission_withdraw_limit')}
            >
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item
              name="commission_withdraw_method"
              label={t('admin.config.commission_withdraw_method')}
            >
              <Select mode="tags" />
            </Form.Item>
            <Form.Item name="withdraw_close_enable" label={t('admin.config.withdraw_close')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
          </div>
        </Card>

        <Card title={t('admin.config.register')} className="mt-4">
          <div className="grid grid-cols-2 gap-x-4">
            <Form.Item name="stop_register" label={t('admin.config.stop_register')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="email_verify" label={t('admin.config.email_verify')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="email_whitelist_enable" label={t('admin.config.email_whitelist_enable')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="email_whitelist_suffix" label={t('admin.config.email_whitelist_suffix')}>
              <Select mode="tags" />
            </Form.Item>
            <Form.Item name="email_gmail_limit_enable" label={t('admin.config.email_gmail_limit')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="recaptcha_enable" label={t('admin.config.recaptcha_enable')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="recaptcha_key" label={t('admin.config.recaptcha_key')}>
              <Input.Password />
            </Form.Item>
            <Form.Item name="recaptcha_site_key" label={t('admin.config.recaptcha_site_key')}>
              <Input />
            </Form.Item>
            <Form.Item
              name="register_limit_by_ip_enable"
              label={t('admin.config.register_limit_by_ip')}
            >
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="register_limit_count" label={t('admin.config.register_limit_count')}>
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item name="register_limit_expire" label={t('admin.config.register_limit_expire')}>
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item name="password_limit_enable" label={t('admin.config.password_limit_enable')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="password_limit_count" label={t('admin.config.password_limit_count')}>
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item name="password_limit_expire" label={t('admin.config.password_limit_expire')}>
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item name="try_out_plan_id" label={t('admin.config.try_out_plan_id')}>
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item name="try_out_hour" label={t('admin.config.try_out_hour')}>
              <InputNumber min={0} />
            </Form.Item>
          </div>
        </Card>

        <Card title={t('admin.config.subscribe')} className="mt-4">
          <div className="grid grid-cols-2 gap-x-4">
            <Form.Item name="allow_new_period" label={t('admin.config.allow_new_period')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="ticket_status" label={t('admin.config.ticket_status')}>
              <InputNumber min={0} max={2} />
            </Form.Item>
            <Form.Item name="safe_mode_enable" label={t('admin.config.safe_mode')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="deposit_bounus" label={t('admin.config.deposit_bonus')}>
              <Select mode="tags" />
            </Form.Item>
            <Form.Item name="available_payment_methods" label={t('admin.config.available_payments')}>
              <Select mode="tags" />
            </Form.Item>
          </div>
        </Card>

        <Card title={t('admin.config.notify')} className="mt-4">
          <div className="grid grid-cols-2 gap-x-4">
            <Form.Item name="telegram_bot_enable" label={t('admin.config.telegram_bot_enable')}>
              <InputNumber min={0} max={1} />
            </Form.Item>
            <Form.Item name="telegram_bot_token" label={t('admin.config.telegram_bot_token')}>
              <Input.Password />
            </Form.Item>
            <Form.Item name="telegram_discuss_link" label={t('admin.config.telegram_discuss_link')}>
              <Input />
            </Form.Item>
          </div>
        </Card>

        <Card title={t('admin.config.theme')} className="mt-4">
          <div className="grid grid-cols-2 gap-x-4">
            <Form.Item name="frontend_theme" label={t('admin.config.frontend_theme')}>
              <Input />
            </Form.Item>
            <Form.Item name="frontend_theme_color" label={t('admin.config.frontend_theme_color')}>
              <Select
                options={[
                  { value: 'default' },
                  { value: 'darkblue' },
                  { value: 'black' },
                  { value: 'green' },
                ]}
              />
            </Form.Item>
            <Form.Item name="frontend_theme_sidebar" label={t('admin.config.frontend_theme_sidebar')}>
              <Select options={[{ value: 'light' }, { value: 'dark' }]} />
            </Form.Item>
            <Form.Item name="frontend_theme_header" label={t('admin.config.frontend_theme_header')}>
              <Select options={[{ value: 'light' }, { value: 'dark' }]} />
            </Form.Item>
            <Form.Item name="frontend_background_url" label={t('admin.config.background_url')}>
              <Input />
            </Form.Item>
          </div>
        </Card>

        <div className="mt-4">
          <Space>
            <Button type="primary" htmlType="submit" loading={save.isPending}>
              {t('common.save')}
            </Button>
          </Space>
        </div>
      </Form>
    </div>
  );
}
