//! The `bash` command every test that runs a bundled helper script starts
//! from.
//!
//! Included by path (`#[path = "helpers/script_cmd.rs"] mod script_cmd;`) so
//! a script test does not compile the whole `helpers` module.
#![allow(dead_code)]

use std::process::Command;

/// `bash` with `GIT_PAW_BROKER_URL` cleared. Inside a live git-paw session
/// the variable names that session's broker, and the helpers prefer it to
/// the test's `[broker]` config, so a test run there would publish to the
/// live session.
#[must_use]
pub fn bash() -> Command {
    let mut cmd = Command::new("bash");
    cmd.env_remove("GIT_PAW_BROKER_URL");
    cmd
}
