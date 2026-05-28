import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { App, Button, Card, Form, Input } from 'antd';
import { passport } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { setAuthData, setSecurePath, getSecurePath } from '@/lib/auth';
import { i18nGet } from '@/lib/errors';

interface FormValues {
  email: string;
  password: string;
  secure_path: string;
}

export default function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { message } = App.useApp();
  const [submitting, setSubmitting] = useState(false);
  const [form] = Form.useForm<FormValues>();

  useEffect(() => {
    const stored = getSecurePath();
    if (stored) form.setFieldValue('secure_path', stored);
  }, [form]);

  const onFinish = async (values: FormValues) => {
    setSubmitting(true);
    try {
      const result = await passport.login(apiClient, {
        email: values.email,
        password: values.password,
      });
      if (!result.is_admin) {
        throw new Error('You do not have admin privileges');
      }
      setAuthData(result.auth_data);
      setSecurePath(values.secure_path.replace(/^\//, ''));
      message.success(t('auth.success_login'));
      navigate('/dashboard', { replace: true });
    } catch (error) {
      if (error instanceof Error) message.error(i18nGet(error.message));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'linear-gradient(135deg,#e6f0fb 0%,#fff 100%)',
        padding: 16,
      }}
    >
      <Card title="V2Board Admin" style={{ width: 380 }}>
        <Form layout="vertical" form={form} onFinish={onFinish}>
          <Form.Item
            name="email"
            label={t('auth.email')}
            rules={[{ required: true, type: 'email' }]}
          >
            <Input autoComplete="email" />
          </Form.Item>
          <Form.Item
            name="password"
            label={t('auth.password')}
            rules={[{ required: true, min: 8 }]}
          >
            <Input.Password autoComplete="current-password" />
          </Form.Item>
          <Form.Item
            name="secure_path"
            label="Secure path"
            tooltip="The admin secure path from your Laravel config (v2board.secure_path)."
            rules={[{ required: true }]}
          >
            <Input placeholder="abc123" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={submitting} block>
            {t('auth.submit_login')}
          </Button>
        </Form>
      </Card>
    </div>
  );
}
