//! Pure ticket policy and value types.

pub const MAX_TICKET_SUBJECT_CHARS: usize = 255;
pub const MAX_TICKET_MESSAGE_BYTES: usize = 65_535;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketLevel {
    Low,
    Medium,
    High,
}

impl TicketLevel {
    pub const fn code(self) -> i16 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
        }
    }
}

impl TryFrom<i16> for TicketLevel {
    type Error = TicketInputViolation;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Medium),
            2 => Ok(Self::High),
            _ => Err(TicketInputViolation::InvalidLevel),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketStatus {
    Open,
    Closed,
}

impl TicketStatus {
    pub const fn code(self) -> i16 {
        match self {
            Self::Open => 0,
            Self::Closed => 1,
        }
    }
}

impl TryFrom<i16> for TicketStatus {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Closed),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketReplyStatus {
    AwaitingOperator,
    Answered,
}

impl TicketReplyStatus {
    pub const fn code(self) -> i16 {
        match self {
            Self::AwaitingOperator => 0,
            Self::Answered => 1,
        }
    }
}

impl TryFrom<i16> for TicketReplyStatus {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::AwaitingOperator),
            1 => Ok(Self::Answered),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketCreationPolicy {
    Open,
    PaidOrderRequired,
    PlanRejected,
    InvalidState,
}

impl From<i32> for TicketCreationPolicy {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Open,
            1 => Self::PaidOrderRequired,
            2 => Self::PlanRejected,
            _ => Self::InvalidState,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketInputViolation {
    EmptySubject,
    SubjectTooLong,
    InvalidLevel,
    EmptyMessage,
    MessageTooLong,
    EmptyWithdrawMethod,
    WithdrawMethodTooLong,
    EmptyWithdrawAccount,
    WithdrawMessageTooLong,
}

pub fn validate_ticket_subject(subject: &str) -> Result<&str, TicketInputViolation> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(TicketInputViolation::EmptySubject);
    }
    if subject.chars().count() > MAX_TICKET_SUBJECT_CHARS {
        return Err(TicketInputViolation::SubjectTooLong);
    }
    Ok(subject)
}

pub fn validate_ticket_message(message: &str) -> Result<&str, TicketInputViolation> {
    let message = message.trim();
    if message.is_empty() {
        return Err(TicketInputViolation::EmptyMessage);
    }
    if message.len() > MAX_TICKET_MESSAGE_BYTES {
        return Err(TicketInputViolation::MessageTooLong);
    }
    Ok(message)
}

/// Operator replies historically permit an empty body, but retain the MySQL
/// TEXT byte ceiling. Keeping this separate prevents the user rule from being
/// accidentally applied to admin/staff and Telegram replies.
pub fn validate_operator_ticket_message(message: &str) -> Result<(), TicketInputViolation> {
    if message.len() > MAX_TICKET_MESSAGE_BYTES {
        Err(TicketInputViolation::MessageTooLong)
    } else {
        Ok(())
    }
}

pub fn validate_ticket_create_input<'a>(
    subject: &'a str,
    level: i16,
    message: &'a str,
) -> Result<(&'a str, TicketLevel, &'a str), TicketInputViolation> {
    Ok((
        validate_ticket_subject(subject)?,
        TicketLevel::try_from(level)?,
        validate_ticket_message(message)?,
    ))
}

pub fn validate_withdrawal_input<'a>(
    method: &'a str,
    account: &'a str,
) -> Result<(&'a str, &'a str, String), TicketInputViolation> {
    let method = method.trim();
    if method.is_empty() {
        return Err(TicketInputViolation::EmptyWithdrawMethod);
    }
    let account = account.trim();
    if account.is_empty() {
        return Err(TicketInputViolation::EmptyWithdrawAccount);
    }
    if method.chars().count() > MAX_TICKET_SUBJECT_CHARS {
        return Err(TicketInputViolation::WithdrawMethodTooLong);
    }
    let message = format!("Withdrawal method：{method}\r\nWithdrawal account：{account}");
    if message.len() > MAX_TICKET_MESSAGE_BYTES {
        return Err(TicketInputViolation::WithdrawMessageTooLong);
    }
    Ok((method, account, message))
}

/// Compares integer commission cents with a decimal yuan threshold without
/// floating point or an infrastructure decimal type. `mantissa` and `scale`
/// are the stable parts exposed by `rust_decimal::Decimal` at the adapter.
pub fn commission_balance_meets_minimum(
    balance_cents: i64,
    minimum_yuan_mantissa: i128,
    minimum_yuan_scale: u32,
) -> bool {
    if minimum_yuan_scale <= 2 {
        let factor = 10_i128.pow(2 - minimum_yuan_scale);
        return minimum_yuan_mantissa
            .checked_mul(factor)
            .is_some_and(|minimum_cents| i128::from(balance_cents) >= minimum_cents);
    }

    // balance >= mantissa / 10^(scale-2). Since balance is integral, compare
    // it with the mathematical ceiling. Rust's truncation already is ceiling
    // for negative values; positive remainders need one extra cent.
    let divisor = 10_i128.pow(minimum_yuan_scale - 2);
    let quotient = minimum_yuan_mantissa / divisor;
    let remainder = minimum_yuan_mantissa % divisor;
    let minimum_whole_cents = quotient + i128::from(remainder > 0);
    i128::from(balance_cents) >= minimum_whole_cents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_match_the_persisted_mysql_shapes() {
        assert!(validate_ticket_subject(&"界".repeat(255)).is_ok());
        assert_eq!(
            validate_ticket_subject(&"界".repeat(256)),
            Err(TicketInputViolation::SubjectTooLong)
        );
        assert!(validate_ticket_message(&"a".repeat(65_535)).is_ok());
        assert_eq!(
            validate_ticket_message(&"a".repeat(65_536)),
            Err(TicketInputViolation::MessageTooLong)
        );
        assert_eq!(
            validate_ticket_message(&"界".repeat(21_846)),
            Err(TicketInputViolation::MessageTooLong)
        );
    }

    #[test]
    fn decimal_minimum_is_compared_exactly_in_integer_cents() {
        assert!(!commission_balance_meets_minimum(1_004, 1005, 2));
        assert!(commission_balance_meets_minimum(1_005, 1005, 2));
        assert!(!commission_balance_meets_minimum(1_005, 10_050_001, 6));
        assert!(commission_balance_meets_minimum(1_006, 10_050_001, 6));
        assert!(!commission_balance_meets_minimum(i64::MAX, i128::MAX, 0));
    }

    #[test]
    fn withdrawal_validation_uses_trimmed_values_and_full_message_bytes() {
        let (method, account, message) = validate_withdrawal_input("  bank  ", "  123  ").unwrap();
        assert_eq!(method, "bank");
        assert_eq!(account, "123");
        assert_eq!(
            message,
            "Withdrawal method：bank\r\nWithdrawal account：123"
        );
    }
}
