export interface AuthData {
  is_admin: boolean;
  auth_data: string;
}

export interface CheckLoginResult {
  is_login: boolean;
  is_admin?: boolean;
}
