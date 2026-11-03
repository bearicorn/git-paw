## MODIFIED Requirements

### Requirement: README Supported AI CLIs table matches `src/detect.rs`

The README's Supported CLIs line SHALL name every CLI binary in `src/detect.rs`'s known-CLI roster, each as a `` `<binary>` `` code span, and SHALL NOT name a binary that the roster does not auto-detect. The mdBook `docs/src/supported-clis.md` table SHALL carry the same binary set.

#### Scenario: README CLI table mentions opencode, cline, and droid

- **WHEN** the README's Supported CLIs line is inspected
- **THEN** it contains the substring `opencode`
- **AND** it contains the substring `cline`
- **AND** it contains the substring `droid`

Test: `tests/supported_clis_docs_sync.rs::readme_and_book_tables_match_known_cli_roster` (`opencode`, `cline`, and `droid` are roster members, so the per-binary missing assertion covers them)

#### Scenario: README and mdBook CLI tables list every roster binary and only those

- **WHEN** the README's Supported CLIs line and `docs/src/supported-clis.md`'s table are inspected
- **THEN** each SHALL contain a `` `<binary>` `` code span for every binary in the known-CLI roster, including `` `cursor-agent` ``
- **AND** neither SHALL contain a `` `cursor` `` binary code span

Test: `tests/supported_clis_docs_sync.rs::readme_and_book_tables_match_known_cli_roster`
