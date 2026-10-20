//! The derived `agent_id` is always one the broker accepts.
//!
//! `agent-id-slug-safety` (`broker-protocol`, "Branch slug function"): every
//! output of [`git_paw::broker::messages::slugify_branch`] has to pass the
//! very predicate the `/publish` handler validates against, never equal the
//! reserved id `supervisor`, and never contain `_`.
//!
//! The test shares the handler's predicate
//! ([`git_paw::broker::server::is_valid_agent_id`]) rather than re-stating the
//! pattern (design D3), so it cannot drift from the handler the way the
//! derivation did — that drift is PP-26: a Spec Kit task naming
//! `ROOM_SETUP.md` produced an agent whose every publish was answered HTTP
//! 400.

use git_paw::broker::messages::slugify_branch;
use git_paw::broker::server::is_valid_agent_id;

#[path = "helpers/slug_corpus.rs"]
mod slug_corpus;

#[test]
fn slugify_output_always_passes_the_publish_validator() {
    for branch in slug_corpus::branch_corpus() {
        let id = slugify_branch(&branch);

        assert!(
            is_valid_agent_id(&id),
            "slugify_branch({branch:?}) = {id:?}, which /publish would reject"
        );
        assert_ne!(
            id, "supervisor",
            "slugify_branch({branch:?}) derived the reserved supervisor id"
        );
        assert!(
            !id.contains('_'),
            "slugify_branch({branch:?}) = {id:?} still contains an underscore"
        );
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "slugify_branch({branch:?}) = {id:?} left a character outside [a-z0-9-]"
        );
    }
}
