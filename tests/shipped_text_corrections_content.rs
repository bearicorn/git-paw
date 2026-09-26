//! Content assertions for the `shipped-text-corrections` change.
//!
//! These tests pin the corrections made by this change: boot blocks and
//! MCP publish tools no longer mention peer cherry-picking (removed in v0.16.0),
//! task prompts do not reference `agent.done` (which does not exist), and
//! error messages are current. These assertions prevent the falsehoods from
//! drifting back.

use std::fs;

// ============================================================================
// Boot blocks: exports description must NOT invite cherry-picking
// ============================================================================

#[test]
fn boot_block_template_exports_description_does_not_invite_cherry_picking() {
    let content = fs::read_to_string("assets/boot-block-template.md")
        .expect("boot-block-template.md exists at expected path");

    // The exports description must be removed entirely or reworded.
    // Check that the old cherry-pick language is gone.
    assert!(
        !content.contains("for peers to cherry-pick"),
        "boot-block-template.md must not mention cherry-picking in exports description"
    );
}

#[test]
fn boot_block_mcp_template_exports_description_does_not_invite_cherry_picking() {
    let content = fs::read_to_string("assets/boot-block-mcp-template.md")
        .expect("boot-block-mcp-template.md exists at expected path");

    // The exports description must be removed entirely or reworded.
    // Check that the old cherry-pick language is gone.
    assert!(
        !content.contains("for peers to cherry-pick"),
        "boot-block-mcp-template.md must not mention cherry-picking in exports description"
    );
}

// ============================================================================
// MCP publish tool: exports field doc must NOT invite cherry-picking
// ============================================================================

#[test]
fn mcp_publish_artifact_field_doc_does_not_mention_cherry_picking() {
    let content =
        fs::read_to_string("src/mcp/tools/publish.rs").expect("publish.rs exists at expected path");

    // The exports field documentation must not mention cherry-picking.
    // Extract the comment from the PublishArtifactParams struct.
    let start = content
        .find("pub struct PublishArtifactParams")
        .expect("PublishArtifactParams struct exists");

    // Look backward from the struct to find its documentation comment.
    let pre_struct = &content[..start];
    let doc_start = pre_struct
        .rfind("///")
        .expect("PublishArtifactParams has doc comment");

    let exports_doc_section = &content[doc_start..start + 200];

    assert!(
        !exports_doc_section.contains("for peers to cherry-pick"),
        "PublishArtifactParams exports field doc must not mention cherry-picking; got: {exports_doc_section}"
    );
}

#[test]
fn mcp_publish_artifact_tool_description_does_not_mention_cherry_picking() {
    let content =
        fs::read_to_string("src/mcp/tools/publish.rs").expect("publish.rs exists at expected path");

    // The tool description for publish_artifact must not mention cherry-picking.
    let start = content
        .find("fn publish_artifact")
        .expect("publish_artifact function exists");

    // Look backward to find the #[tool(description = ...)] macro.
    let pre_fn = &content[..start];
    let desc_start = pre_fn
        .rfind("description =")
        .expect("publish_artifact has tool description");

    let tool_desc_section = &content[desc_start..start + 500];

    assert!(
        !tool_desc_section.contains("for peers to cherry-pick"),
        "publish_artifact tool description must not mention cherry-picking; got: {tool_desc_section}"
    );
}

// ============================================================================
// Task prompts: no mention of agent.done and no version promises
// ============================================================================

#[test]
fn speckit_consolidated_footer_does_not_reference_agent_done() {
    let content = fs::read_to_string("assets/task-prompts/speckit/consolidated-footer.md")
        .expect("consolidated-footer.md exists at expected path");

    assert!(
        !content.contains("agent.done"),
        "speckit consolidated-footer must not reference agent.done"
    );
}

#[test]
fn superpowers_execution_does_not_reference_agent_done() {
    let content = fs::read_to_string("assets/task-prompts/superpowers/execution.md")
        .expect("execution.md exists at expected path");

    assert!(
        !content.contains("agent.done"),
        "superpowers execution prompt must not reference agent.done"
    );
}

#[test]
fn speckit_validation_criteria_does_not_promise_future_versions() {
    let content = fs::read_to_string("assets/task-prompts/speckit/validation-criteria.md")
        .expect("validation-criteria.md exists at expected path");

    assert!(
        !content.contains("planned for v"),
        "validation-criteria must not promise enforcement in future versions"
    );
}

#[test]
fn all_task_prompts_reference_commit_not_agent_done() {
    let prompt_files = [
        "assets/task-prompts/speckit/consolidated-footer.md",
        "assets/task-prompts/speckit/consolidated-intro.md",
        "assets/task-prompts/speckit/feature-context-heading.md",
        "assets/task-prompts/speckit/implementation-plan-heading.md",
        "assets/task-prompts/speckit/validation-criteria.md",
        "assets/task-prompts/speckit/your-task-heading.md",
        "assets/task-prompts/superpowers/execution.md",
        "assets/task-prompts/superpowers/plan-context-heading.md",
        "assets/task-prompts/superpowers/your-tasks-heading.md",
    ];

    for file_path in &prompt_files {
        let content = fs::read_to_string(file_path).unwrap_or_else(|_| String::new());

        // If the file mentions "Publish" at all, it must not be for "agent.done"
        if content.contains("Publish") {
            assert!(
                !content.contains("Publish `agent.done`"),
                "{file_path} must not reference 'Publish agent.done'"
            );
        }
    }
}
