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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoneyMinorError {
    OutOfRange(i64),
}

impl fmt::Display for MoneyMinorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        assert_eq!(MoneyMinor::try_from(0).unwrap(), MoneyMinor::ZERO);
        assert_eq!(MoneyMinor::try_from(1_999).unwrap().get(), 1_999);
        assert_eq!(MoneyMinor::try_from(-1).unwrap().get(), -1);
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
}
