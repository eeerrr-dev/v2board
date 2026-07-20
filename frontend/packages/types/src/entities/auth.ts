export interface AuthData {
  is_admin: boolean;
  auth_data: string;
}

export interface CheckLoginResult {
  is_login: boolean;
  is_admin?: boolean;
  /** §6.12: present (with `admin_permissions`) exactly for staff sessions. */
  is_staff?: boolean;
  /** §6.12 staff grants (`{family}:read|write`); may be an empty array. */
  admin_permissions?: string[];
}
