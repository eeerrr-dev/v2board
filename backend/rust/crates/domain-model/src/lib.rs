//! Pure business concepts shared by application services and adapters.
//!
//! This crate deliberately has no database, cache, HTTP, configuration, or
//! async-runtime dependencies. Infrastructure translates at its boundary;
//! business vocabulary and invariant checks live here.

mod money;
mod plan;

pub use money::{MoneyMinor, MoneyMinorError};
pub use plan::{PlanPricePeriod, PlanPriceUpdate, PlanPriceUpdates, PlanPrices};
