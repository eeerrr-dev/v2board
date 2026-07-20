use std::fmt;

/// A signed amount in the currency's minor unit (for example cents).
///
/// PostgreSQL stores this value as `INTEGER`; accepting an `i64` at transport
/// boundaries prevents lossy casts before validation. Plan prices deliberately
/// retain the legacy administrator/import contract's signed range.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MoneyMinor(i32);

impl MoneyMinor {
    pub const ZERO: Self = Self(0);

    pub const fn from_i32(value: i32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i32 {
        self.0
    }
}

impl From<MoneyMinor> for i32 {
    fn from(value: MoneyMinor) -> Self {
        value.get()
    }
}

impl TryFrom<i64> for MoneyMinor {
    type Error = MoneyMinorError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        i32::try_from(value)
            .map(Self)
            .map_err(|_| MoneyMinorError::OutOfRange(value))
    }
}

/// A balance, charge, or payout in minor units.
///
/// Plan administration deliberately uses [`MoneyMinor`] because its imported
/// contract is signed. Runtime money must use this narrower type so a negative
/// amount cannot cross a business-policy boundary unnoticed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NonNegativeMoneyMinor(i32);

impl NonNegativeMoneyMinor {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: i32) -> Result<Self, MoneyMinorError> {
        if value < 0 {
            Err(MoneyMinorError::Negative(value as i64))
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> i32 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, MoneyMinorError> {
        let value = i64::from(self.0) + i64::from(other.0);
        Self::try_from(value)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0
            .checked_sub(other.0)
            .and_then(|value| Self::new(value).ok())
    }
}

impl From<NonNegativeMoneyMinor> for i32 {
    fn from(value: NonNegativeMoneyMinor) -> Self {
        value.get()
    }
}

impl TryFrom<i64> for NonNegativeMoneyMinor {
    type Error = MoneyMinorError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value < 0 {
            return Err(MoneyMinorError::Negative(value));
        }
        i32::try_from(value)
            .map(Self)
            .map_err(|_| MoneyMinorError::OutOfRange(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoneyMinorError {
    Negative(i64),
    OutOfRange(i64),
}

impl fmt::Display for MoneyMinorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Negative(value) => {
                write!(formatter, "minor-unit amount {value} must not be negative")
            }
            Self::OutOfRange(value) => {
                write!(formatter, "minor-unit amount {value} exceeds INTEGER range")
            }
        }
    }
}

impl std::error::Error for MoneyMinorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_minor_preserves_the_signed_postgres_integer_range() {
        assert_eq!(MoneyMinor::from_i32(0), MoneyMinor::ZERO);
        assert_eq!(MoneyMinor::from_i32(1_999).get(), 1_999);
        assert_eq!(MoneyMinor::from_i32(-1).get(), -1);
        assert_eq!(
            MoneyMinor::try_from(i64::from(i32::MIN)).unwrap().get(),
            i32::MIN
        );
        assert!(matches!(
            MoneyMinor::try_from(i64::from(i32::MAX) + 1),
            Err(MoneyMinorError::OutOfRange(_))
        ));
        assert!(matches!(
            MoneyMinor::try_from(i64::from(i32::MIN) - 1),
            Err(MoneyMinorError::OutOfRange(_))
        ));
    }

    #[test]
    fn runtime_money_rejects_negative_values_and_checks_arithmetic() {
        assert_eq!(
            NonNegativeMoneyMinor::new(0).unwrap(),
            NonNegativeMoneyMinor::ZERO
        );
        assert!(matches!(
            NonNegativeMoneyMinor::new(-1),
            Err(MoneyMinorError::Negative(-1))
        ));
        let one = NonNegativeMoneyMinor::new(1).unwrap();
        assert_eq!(one.checked_add(one).unwrap().get(), 2);
        assert_eq!(one.checked_sub(one), Some(NonNegativeMoneyMinor::ZERO));
        assert_eq!(
            one.checked_sub(NonNegativeMoneyMinor::new(2).unwrap()),
            None
        );
        assert!(
            NonNegativeMoneyMinor::new(i32::MAX)
                .unwrap()
                .checked_add(one)
                .is_err()
        );
    }
}
