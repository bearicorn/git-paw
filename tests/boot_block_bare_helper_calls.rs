//! `boot-block`'s *Boot block asks for bare helper calls*: both rendered boot
//! blocks ask for each `broker.sh` call to be run bare, so one path-scoped
//! permission rule matches every call.

use git_paw::skills::build_boot_block;

const BROKER_URL: &str = "http://127.0.0.1:9119";

fn assert_asks_for_bare_calls(block: &str, variant: &str) {
    // The rule leads the block: the first paragraph after the heading.
    let first = block
        .split("\n\n")
        .find(|p| !p.trim().is_empty() && !p.trim_start().starts_with("## "))
        .unwrap_or_default();
    assert!(
        first.contains("Run each helper call bare"),
        "{variant} boot block should open with the bare-call rule; first paragraph:\n{first}"
    );
    assert!(
        block.contains("no pipe"),
        "{variant} boot block should rule out piping a helper call"
    );
}

/// Scenario: Standard boot block asks for bare helper calls.
#[test]
fn standard_boot_block_asks_for_bare_helper_calls() {
    let block = build_boot_block("feat/test", BROKER_URL, false);
    assert_asks_for_bare_calls(&block, "standard");
    for line in block.lines().filter(|l| l.contains("broker.sh --agent")) {
        assert!(
            !line.contains("; echo") && !line.contains(" | "),
            "a boot-block helper example must be bare: {line}"
        );
    }
}

/// Scenario: MCP boot block asks for bare helper calls.
#[test]
fn mcp_boot_block_asks_for_bare_helper_calls() {
    let block = build_boot_block("feat/test", BROKER_URL, true);
    assert_asks_for_bare_calls(&block, "MCP");
}

/// Scenario: The instruction names no agent CLI.
#[test]
fn bare_call_instruction_names_no_agent_cli() {
    for mcp in [false, true] {
        let block = build_boot_block("feat/test", BROKER_URL, mcp);
        let paragraph = block
            .split("\n\n")
            .find(|p| p.contains("call bare"))
            .unwrap_or_else(|| panic!("boot block (mcp={mcp}) has a bare-call paragraph"))
            .to_lowercase();
        for cli in ["claude", "codex", "gemini", "aider", "cursor", "copilot"] {
            assert!(
                !paragraph.contains(cli),
                "the bare-call instruction (mcp={mcp}) should name no agent CLI ({cli})"
            );
        }
    }
}
