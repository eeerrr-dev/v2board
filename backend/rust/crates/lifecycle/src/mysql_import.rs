mod clickhouse_target;
mod copy_stream;
mod execute;
mod mysql_source;
mod postgres_acl_registry;
mod postgres_grants;
mod postgres_target;
mod redis_target;
mod target_verify;

#[cfg(test)]
mod tests;

pub(crate) use execute::execute;

#[cfg(test)]
use {
    clickhouse_target::*, copy_stream::*, execute::*, mysql_source::*, postgres_target::*,
    redis_target::*, target_verify::*,
};
