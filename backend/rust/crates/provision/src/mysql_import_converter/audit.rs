use std::collections::BTreeSet;

use super::mappings::{
    DERIVED_MAPPINGS, DISCARDED_SOURCE_TABLES, DISCARDED_TARGET_TABLES, SCALAR_REFERENCES,
    TABLE_MAPPINGS,
};
use super::values::ConverterError;

pub fn audit_registry() -> Result<(), ConverterError> {
    let expected_source_tables = [
        "v2_commission_log",
        "v2_coupon",
        "v2_giftcard",
        "v2_invite_code",
        "v2_knowledge",
        "v2_notice",
        "v2_order",
        "v2_payment",
        "v2_plan",
        "v2_server_group",
        "v2_stat",
        "v2_ticket",
        "v2_ticket_message",
        "v2_user",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let expected_target_tables = [
        "commission_log",
        "coupon",
        "gift_card",
        "invite_code",
        "knowledge",
        "notice",
        "orders",
        "payment_method",
        "plan",
        "server_group",
        "stat",
        "ticket",
        "ticket_message",
        "users",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let expected_derived_target_tables = ["gift_card_redemption", "plan_price"]
        .into_iter()
        .collect::<BTreeSet<_>>();

    let mut seen_orders = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut seen_targets = BTreeSet::new();
    for mapping in TABLE_MAPPINGS {
        validate_identifier(mapping.source)?;
        validate_identifier(mapping.target)?;
        if !seen_orders.insert(mapping.order) {
            return registry_error(format!("duplicate table order {}", mapping.order));
        }
        if !seen_sources.insert(mapping.source) {
            return registry_error(format!("duplicate source table {}", mapping.source));
        }
        if !seen_targets.insert(mapping.target) {
            return registry_error(format!("duplicate target table {}", mapping.target));
        }

        let mut source_columns = BTreeSet::new();
        let mut target_columns = BTreeSet::new();
        for column in mapping.direct_columns {
            validate_identifier(column)?;
            if !source_columns.insert(*column) || !target_columns.insert(*column) {
                return registry_error(format!(
                    "duplicate direct column {}.{}",
                    mapping.source, column
                ));
            }
        }
        for column in mapping.transformed_columns {
            validate_identifier(column.source)?;
            validate_identifier(column.target)?;
            if !source_columns.insert(column.source) || !target_columns.insert(column.target) {
                return registry_error(format!(
                    "duplicate transformed column {}.{}",
                    mapping.source, column.source
                ));
            }
        }
        for column in mapping.added_columns {
            validate_identifier(column.target)?;
            if !target_columns.insert(column.target) {
                return registry_error(format!(
                    "duplicate added target column {}.{}",
                    mapping.target, column.target
                ));
            }
            if column.provenance.trim().is_empty() {
                return registry_error(format!(
                    "added target column {}.{} lacks provenance",
                    mapping.target, column.target
                ));
            }
        }
        for column in mapping.consumed_source_columns {
            validate_identifier(column.source)?;
            if !source_columns.insert(column.source) {
                return registry_error(format!(
                    "duplicate consumed source column {}.{}",
                    mapping.source, column.source
                ));
            }
        }
        if !source_columns.contains("id") || !target_columns.contains("id") {
            return registry_error(format!(
                "base mapping {} must preserve its id",
                mapping.source
            ));
        }
        if !mapping.direct_columns.contains(&"created_at")
            || !mapping.direct_columns.contains(&"updated_at")
        {
            return registry_error(format!(
                "base mapping {} must preserve both timestamps",
                mapping.source
            ));
        }
    }

    if seen_sources != expected_source_tables {
        return registry_error("source table registry differs from the pinned core inventory");
    }
    if seen_targets != expected_target_tables {
        return registry_error(
            "target table registry differs from the unprefixed native inventory",
        );
    }
    if !TABLE_MAPPINGS
        .windows(2)
        .all(|pair| pair[0].order < pair[1].order)
    {
        return registry_error("table mappings are not stored in strict execution order");
    }
    for mapping in TABLE_MAPPINGS {
        for column in mapping.transformed_columns {
            match (
                column.source_referenced_table,
                column.referenced_target_table,
            ) {
                (None, None) => {}
                (Some(source_table), Some(target_table))
                    if TABLE_MAPPINGS.iter().any(|candidate| {
                        candidate.source == source_table && candidate.target == target_table
                    }) => {}
                _ => {
                    return registry_error(format!(
                        "{}.{} has mismatched source/target reference metadata",
                        mapping.source, column.source
                    ));
                }
            }
        }
    }
    if DERIVED_MAPPINGS
        .iter()
        .any(|mapping| !seen_orders.insert(mapping.order))
    {
        return registry_error("derived mapping order collides with a base mapping");
    }
    if !DERIVED_MAPPINGS
        .windows(2)
        .all(|pair| pair[0].order < pair[1].order)
    {
        return registry_error("derived mappings are not stored in strict execution order");
    }
    let mut derived_targets = BTreeSet::new();
    let mut derived_stream_owners = BTreeSet::new();
    for mapping in DERIVED_MAPPINGS {
        validate_identifier(mapping.target)?;
        if mapping.source_tables.is_empty()
            || mapping.target_columns.is_empty()
            || mapping.key_columns.is_empty()
        {
            return registry_error(format!(
                "derived mapping {} has no source, target columns, or key",
                mapping.target
            ));
        }
        if seen_targets.contains(mapping.target) || !derived_targets.insert(mapping.target) {
            return registry_error(format!(
                "derived target {} collides with another target",
                mapping.target
            ));
        }
        let stream_owner = mapping.source_tables[0];
        if !derived_stream_owners.insert(stream_owner) {
            return registry_error(format!(
                "source {stream_owner} owns more than one derived COPY stream; the single-source executor requires exactly one owner"
            ));
        }
        for source in mapping.source_tables {
            validate_identifier(source)?;
            if !seen_sources.contains(source) {
                return registry_error(format!(
                    "derived mapping {} has unknown source {source}",
                    mapping.target
                ));
            }
        }
        let mut target_columns = BTreeSet::new();
        for column in mapping.target_columns {
            validate_identifier(column)?;
            if !target_columns.insert(*column) {
                return registry_error(format!(
                    "derived mapping {} repeats target column {column}",
                    mapping.target
                ));
            }
        }
        for column in mapping.key_columns {
            validate_identifier(column)?;
            if !target_columns.contains(column) {
                return registry_error(format!(
                    "derived mapping {} key column {column} is not a target column",
                    mapping.target
                ));
            }
        }
    }
    if derived_targets != expected_derived_target_tables {
        return registry_error(
            "derived target registry differs from the normalized native inventory",
        );
    }
    for reference in SCALAR_REFERENCES {
        validate_identifier(reference.source_table)?;
        validate_identifier(reference.target_table)?;
        validate_identifier(reference.column)?;
        validate_identifier(reference.source_referenced_table)?;
        validate_identifier(reference.target_referenced_table)?;
        if !seen_sources.contains(reference.source_table)
            || !seen_sources.contains(reference.source_referenced_table)
            || !seen_targets.contains(reference.target_table)
            || !seen_targets.contains(reference.target_referenced_table)
            || !TABLE_MAPPINGS.iter().any(|mapping| {
                mapping.source == reference.source_table && mapping.target == reference.target_table
            })
            || !TABLE_MAPPINGS.iter().any(|mapping| {
                mapping.source == reference.source_referenced_table
                    && mapping.target == reference.target_referenced_table
            })
        {
            return registry_error(format!(
                "scalar reference {} -> {}.{} names an unregistered source or target table",
                reference.source_table, reference.target_table, reference.column
            ));
        }
    }
    let discarded_sources = DISCARDED_SOURCE_TABLES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if discarded_sources.len() != DISCARDED_SOURCE_TABLES.len()
        || !discarded_sources.is_disjoint(&seen_sources)
    {
        return registry_error("schema-v1 discard inventory is not the audited policy");
    }
    for table in DISCARDED_SOURCE_TABLES {
        validate_identifier(table)?;
    }
    let discarded_targets = DISCARDED_TARGET_TABLES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if discarded_targets.len() != DISCARDED_TARGET_TABLES.len()
        || !discarded_targets.is_disjoint(&seen_targets)
    {
        return registry_error("schema-v1 discarded target inventory is not unique and disjoint");
    }
    for table in DISCARDED_TARGET_TABLES {
        validate_identifier(table)?;
    }
    if ["v2_user", "v2_payment", "v2_server_group", "v2_stat"]
        .iter()
        .any(|table| discarded_sources.contains(table))
    {
        return registry_error("schema-v1 discards protected durable business data");
    }
    Ok(())
}

pub(super) fn validate_identifier(identifier: &str) -> Result<(), ConverterError> {
    let valid = !identifier.is_empty()
        && identifier
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        registry_error(format!("unsafe SQL identifier {identifier:?}"))
    }
}

pub(super) fn registry_error<T>(message: impl Into<String>) -> Result<T, ConverterError> {
    Err(ConverterError::Registry(message.into()))
}
