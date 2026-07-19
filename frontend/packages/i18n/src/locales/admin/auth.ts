// Admin login, two-factor (TOTP) management, and step-up re-auth copy. Values
// stay Chinese until product translations are supplied (see ./index.ts).
export const adminAuth = {
  // Login page. The `email_required`/`email_invalid`/`password_min` keys are
  // also referenced as flat runtime strings by the login zod schema and
  // resolved through FieldError's translateRuntimeMessage.
  email_required: '请输入邮箱',
  email_invalid: '请输入有效邮箱',
  password_min: '密码至少需要 8 个字符',
  not_admin: '无管理员权限',
  mfa_code_invalid: '验证码错误或已被使用',
  login_description: '登录到管理中心',
  email: '邮箱',
  password: '密码',
  mfa_code_label: '两步验证码',
  mfa_code_placeholder: '验证器 App 中的 6 位验证码',
  sign_in: '登入',
  forgot_password: '忘记密码',
  forgot_description: '在站点目录下执行命令找回密码',
  reset_password_command:
    "V2BOARD_NEW_PASSWORD='新密码' v2board-api reset-admin-password 管理员邮箱",
  got_it: '我知道了',
  // Account two-factor dialog.
  operation_failed: '操作失败，请稍后重试',
  mfa_title: '两步验证',
  mfa_description:
    '使用 TOTP 验证器 App（如 Google Authenticator、1Password）为管理账号增加第二重保护。',
  mfa_enabled: '两步验证已启用',
  mfa_disabled: '两步验证已关闭',
  copy_success: '复制成功',
  mfa_disable_label: '输入当前验证码以关闭',
  code_placeholder: '6 位验证码',
  mfa_disable_submit: '关闭两步验证',
  mfa_manual_secret: '无法扫码时，手动输入密钥：',
  mfa_confirm_label: '输入 App 中的验证码完成绑定',
  mfa_confirm_submit: '确认并启用',
  // The ASCII space after '；' reproduces the JSX multi-line text join of the
  // original copy byte-for-byte.
  mfa_intro:
    '当前账号未启用两步验证。启用后，登录除密码外还需输入验证器 App 中的动态验证码； 如手机遗失，可由服务器操作员执行',
  mfa_reset_command: 'v2board-api reset-admin-totp 邮箱',
  mfa_intro_suffix: '解除。',
  mfa_setup_start: '生成密钥并开始设置',
  // Step-up re-auth dialog.
  step_up_success: '验证成功，请重试刚才的操作',
  step_up_failed: '验证失败，请稍后再试',
  step_up_title: '验证管理员密码',
  step_up_description: '此操作需要重新验证您的登录密码。',
  current_password: '当前密码',
  verify: '验证',
  // Forced-MFA enrollment gate (admin shell).
  mfa_required_title: '需要启用两步验证',
  mfa_required_description: '本站点已强制要求管理人员启用两步验证，完成绑定后即可继续使用后台。',
  mfa_setup_now: '立即设置',
};
