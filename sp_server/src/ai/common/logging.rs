/// NPC/AI Logging Helpers
///
/// This module provides structured logging macros for NPC and AI systems.
/// All logs include entity identification for easier debugging and filtering.

use bevy::prelude::*;

/// Extracts a display-friendly identifier from an entity and optional ID component.
/// Returns a string like "Entity(123v4)/ID(456)" or just "Entity(123v4)" if no ID.
pub fn entity_display(entity: Entity, obj_id: Option<i32>) -> String {
    match obj_id {
        Some(id) => format!("E{}/ID{}", entity.index(), id),
        None => format!("E{}", entity.index()),
    }
}

/// Macro for NPC info-level logging with entity context.
/// Usage: npc_info!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! npc_info {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        info!(
            target: "siege_perilous::npc",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for NPC debug-level logging with entity context.
/// Usage: npc_debug!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! npc_debug {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        debug!(
            target: "siege_perilous::npc",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for NPC error-level logging with entity context.
/// Usage: npc_error!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! npc_error {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        error!(
            target: "siege_perilous::npc",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for NPC warn-level logging with entity context.
/// Usage: npc_warn!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! npc_warn {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        warn!(
            target: "siege_perilous::npc",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for NPC trace-level logging with entity context.
/// Usage: npc_trace!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! npc_trace {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        trace!(
            target: "siege_perilous::npc",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for villager info-level logging with entity context.
/// Usage: villager_info!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! villager_info {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        info!(
            target: "siege_perilous::villager",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for villager debug-level logging with entity context.
/// Usage: villager_debug!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! villager_debug {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        debug!(
            target: "siege_perilous::villager",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for villager error-level logging with entity context.
/// Usage: villager_error!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! villager_error {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        error!(
            target: "siege_perilous::villager",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for villager warn-level logging with entity context.
/// Usage: villager_warn!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! villager_warn {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        warn!(
            target: "siege_perilous::villager",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Macro for villager trace-level logging with entity context.
/// Usage: villager_trace!(entity, obj_id, "message {}", arg);
#[macro_export]
macro_rules! villager_trace {
    ($entity:expr, $obj_id:expr, $($arg:tt)*) => {
        trace!(
            target: "siege_perilous::villager",
            entity = %$crate::ai_logging::entity_display($entity, $obj_id),
            $($arg)*
        )
    };
}

/// Helper to execute logging within a span context.
/// This ensures all logs are properly associated with the scorer/action span.
///
/// Usage:
/// ```ignore
/// with_span!(span, {
///     npc_info!(entity, Some(id), "Processing target");
///     // ... more code
/// });
/// ```
#[macro_export]
macro_rules! with_span {
    ($span:expr, $block:block) => {
        $span.in_scope(|| $block)
    };
}
