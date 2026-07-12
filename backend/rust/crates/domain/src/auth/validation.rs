use v2board_compat::ApiError;

pub(super) const MAX_EMAIL_CHARS: usize = 64;
pub(super) const MAX_PASSWORD_CHARS: usize = 128;

/// Laravel `AuthRegister`/`CommSendEmailVerify` validate `email => required|email:strict`.
/// The FormRequest fires before the controller body, returning HTTP 422 with the field message.
pub(super) fn validate_email(email: &str) -> Result<(), ApiError> {
    let email = email.trim();
    if email.is_empty() {
        return Err(ApiError::validation_field(
            "email",
            "Email can not be empty",
        ));
    }
    if !is_valid_email(email) {
        return Err(ApiError::validation_field(
            "email",
            "Email format is incorrect",
        ));
    }
    if email.chars().count() > MAX_EMAIL_CHARS {
        return Err(ApiError::validation_field(
            "email",
            "Email format is incorrect",
        ));
    }
    Ok(())
}

/// Authentication identifiers use one canonical cache/rate-limit spelling.
/// MySQL's email uniqueness lookup is case-insensitive in the native schema,
/// so the same spelling is safe for the database lookup as well.
pub(super) fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

/// Laravel `AuthRegister` validates `password => required|min:8` (character count, not bytes).
pub(super) fn validate_password(password: &str) -> Result<(), ApiError> {
    if password.is_empty() {
        return Err(ApiError::validation_field(
            "password",
            "Password can not be empty",
        ));
    }
    if password.chars().count() < 8 {
        return Err(ApiError::validation_field(
            "password",
            "Password must be greater than 8 digits",
        ));
    }
    if password.chars().count() > MAX_PASSWORD_CHARS {
        return Err(ApiError::validation_field(
            "password",
            "Password must be greater than 8 digits",
        ));
    }
    Ok(())
}

/// Laravel `AuthForget` validates email (`required|string|email:strict|max:64`),
/// password (`required|string|min:8|max:64`) and email_code (`required|string|digits:6`).
/// Lengths are character counts (`mb_strlen`), not bytes. Fires before the controller body,
/// returning HTTP 422 with the field message.
pub(super) fn validate_forget(
    email: &str,
    password: &str,
    email_code: &str,
) -> Result<(), ApiError> {
    validate_email(email)?;
    if email.trim().chars().count() > 64 {
        return Err(ApiError::validation_field(
            "email",
            "Email format is incorrect",
        ));
    }
    validate_password(password)?;
    if password.chars().count() > 64 {
        return Err(ApiError::validation_field(
            "password",
            "Password must be greater than 8 digits",
        ));
    }
    if email_code.trim().is_empty() {
        return Err(ApiError::validation_field(
            "email_code",
            "Email verification code cannot be empty",
        ));
    }
    if email_code.chars().count() != 6 || !email_code.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(ApiError::validation_field(
            "email_code",
            "Incorrect email verification code",
        ));
    }
    Ok(())
}

/// Laravel `UserChangePassword` validates old_password (`required`) and new_password
/// (`required|min:8`, character count not bytes). The FormRequest fires before the
/// controller body, returning HTTP 422 with the field message.
pub(super) fn validate_change_password(
    old_password: &str,
    new_password: &str,
) -> Result<(), ApiError> {
    if old_password.is_empty() {
        return Err(ApiError::validation_field(
            "old_password",
            "Old password cannot be empty",
        ));
    }
    if old_password.chars().count() > MAX_PASSWORD_CHARS {
        return Err(ApiError::validation_field(
            "old_password",
            "The old password is wrong",
        ));
    }
    if new_password.is_empty() {
        return Err(ApiError::validation_field(
            "new_password",
            "New password cannot be empty",
        ));
    }
    if new_password.chars().count() < 8 {
        return Err(ApiError::validation_field(
            "new_password",
            "Password must be greater than 8 digits",
        ));
    }
    if new_password.chars().count() > MAX_PASSWORD_CHARS {
        return Err(ApiError::validation_field(
            "new_password",
            "Password must be greater than 8 digits",
        ));
    }
    Ok(())
}

/// Structural `local@host` check — the practical subset of `email:strict` that avoids
/// false-rejecting any address Laravel's RFC validator would accept in real registrations.
pub(super) fn is_valid_email(email: &str) -> bool {
    if email.chars().any(char::is_whitespace) {
        return false;
    }
    match email.split_once('@') {
        Some((local, host)) => !local.is_empty() && !host.is_empty() && !host.contains('@'),
        None => false,
    }
}
