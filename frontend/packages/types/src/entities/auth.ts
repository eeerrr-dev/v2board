export interface AuthData {
  is_admin: 0 | 1;
  auth_data: string;
}

export interface CheckLoginResult {
  is_login: boolean;
  is_admin?: boolean;
}
