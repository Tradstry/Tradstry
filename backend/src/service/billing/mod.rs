//! Plan entitlements and usage limits.
//!
//! Tiers (free / pro / pro_plus) differ only by numeric limits — there is no
//! feature gating. Limits come from the `plan_limits` table, never from code, so
//! changing a tier is an UPDATE rather than a deploy.
//!
//! See `docs/superpowers/specs/2026-07-18-plan-entitlements-paddle-design.md`.

pub mod entitlements;
pub mod paddle;
pub mod portal;
pub mod quota;
pub mod rate_limit;
pub mod usage;
pub mod usage_sweep;
pub mod worker;
