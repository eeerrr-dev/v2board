import type { Translations } from './zh-CN';

// Localized backend error-message strings (the `errors` slice of the old full
// translation tree). Every locale renders its UI copy at runtime from the
// zh-CN tree plus the legacy dictionaries (see ../index.ts), so this slice —
// consumed by the user app's error toasts — is the only per-locale tree data
// still bundled.
const enUSErrors: Translations['errors'] = {
  '请求失败': 'Request failed',
  network: 'Network error, please try again later',
  'Incorrect email or password': 'Incorrect email or password',
  'Your account has been suspended': 'Your account has been suspended',
  'Token error': 'Token error',
  'This email is registered': 'This email is registered',
  'This email is not registered in the system': 'This email is not registered in the system',
  'Email verification code has been sent, please request again later':
    'Email verification code has been sent, please request again later',
  'Email verification code': 'Email verification code',
  'Email already exists': 'Email already exists',
  'Invalid invitation code': 'Invalid invitation code',
  'Register failed': 'Register failed',
  'Email suffix is not in the Whitelist': 'Email suffix is not in the Whitelist',
  'Gmail alias is not supported': 'Gmail alias is not supported',
  'Registration has closed': 'Registration has closed',
  'You must use the invitation code to register': 'You must use the invitation code to register',
  'Email verification code cannot be empty': 'Email verification code cannot be empty',
  'Incorrect email verification code': 'Incorrect email verification code',
  'Invalid code is incorrect': 'Invalid code is incorrect',
  'Register frequently, please try again after :minute minute':
    'Register frequently, please try again after {{minute}} minute',
  'There are too many password errors, please try again after :minute minutes.':
    'There are too many password errors, please try again after {{minute}} minutes.',
  'Reset failed': 'Reset failed',
  'Reset failed, Please try again later': 'Reset failed, please try again later',
  'The user does not exist': 'The user does not exist',
  'The old password is wrong': 'The old password is wrong',
  'Save failed': 'Save failed',
  'Subscription plan does not exist': 'Subscription plan does not exist',
  'Invalid parameter': 'Invalid parameter',
  'Insufficient commission balance': 'Insufficient commission balance',
  'Transfer failed': 'Transfer failed',
  'Ticket does not exist': 'Ticket does not exist',
  'There are other unresolved tickets': 'There are other unresolved tickets',
  'Failed to open ticket': 'Failed to open ticket',
  'Message cannot be empty': 'Message cannot be empty',
  'The ticket is closed and cannot be replied': 'The ticket is closed and cannot be replied',
  'Please wait for the technical enginneer to reply':
    'Please wait for the technical engineer to reply',
  'Ticket reply failed': 'Ticket reply failed',
  'Close failed': 'Close failed',
  'Unsupported withdrawal method': 'Unsupported withdrawal method',
  'The current required minimum withdrawal commission is :limit':
    'The current minimum withdrawal commission is {{limit}}',
  'Order does not exist': 'Order does not exist',
  'You have an unpaid or pending order, please try again later or cancel it':
    'You have an unpaid or pending order, please try again later or cancel it',
  'This subscription has been sold out, please choose another subscription':
    'This subscription has been sold out, please choose another subscription',
  'This subscription cannot be renewed, please change to another subscription':
    'This subscription cannot be renewed, please change to another subscription',
  'This payment period cannot be purchased, please choose another period':
    'This payment period cannot be purchased, please choose another period',
  'Subscription has expired or no active subscription, unable to purchase Data Reset Package':
    'Subscription has expired or no active subscription, unable to purchase Data Reset Package',
  'This subscription has expired, please change to another subscription':
    'This subscription has expired, please change to another subscription',
  'Coupon failed': 'Coupon failed',
  'Insufficient balance': 'Insufficient balance',
  'Failed to create order': 'Failed to create order',
  'Order does not exist or has been paid': 'Order does not exist or has been paid',
  'Payment method is not available': 'Payment method is not available',
  'You can only cancel pending orders': 'You can only cancel pending orders',
  'Cancel failed': 'Cancel failed',
  'Payment gateway request failed': 'Payment gateway request failed',
  'Article does not exist': 'Article does not exist',
  'You must have a valid subscription to view content in this area':
    'You must have a valid subscription to view content in this area',
  'The maximum number of creations has been reached':
    'The maximum number of creations has been reached',
  'Coupon cannot be empty': 'Coupon cannot be empty',
  'This coupon is no longer available': 'This coupon is no longer available',
  'This coupon has not yet started': 'This coupon has not yet started',
  'This coupon has expired': 'This coupon has expired',
  'The coupon code cannot be used for this subscription':
    'The coupon code cannot be used for this subscription',
  'Invalid coupon': 'Invalid coupon',
  'Current product is sold out': 'Current product is sold out',
  'Request failed, please try again later': 'Request failed, please try again later',
  "Oops, there's a problem... Please refresh the page and try again later":
    "Oops, there's a problem... Please refresh the page and try again later",
  'Payment failed. Please check your credit card information':
    'Payment failed. Please check your credit card information',
};

export default enUSErrors;
