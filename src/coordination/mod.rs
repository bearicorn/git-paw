//! User→agent coordination for the supervisor routing commands.
//!
//! This module is the home of the reusable inventory + target-validation
//! helpers (design D6) that back the `/agents` and `/tell` supervisor skills.
//! v0.6.0 has a single consumer — the `/tell` skill — but the helpers are
//! shaped as library functions so the v1.0.0 MCP write tools' analogous
//! `publish_agent_feedback` can adopt the same inventory shape and unknown-
//! target rejection semantics without re-implementation.
//!
//! Distinct from [`crate::broker`] peer-to-peer coordination: that surface is
//! agent↔agent; this one is user→agent, mediated by the supervisor.

pub mod inventory;
pub mod tell;
