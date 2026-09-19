## MODIFIED Requirements

### Requirement: sweep.sh detects stuck-on-prompt agents

The bundled `assets/scripts/sweep.sh` helper SHALL detect a
"stuck on prompt" state for each agent pane by inspecting
recent `tmux capture-pane` output. The helper SHALL flag a
pane as stuck when its capture contains documented prompt
markers AND the agent's broker `last_seen_seconds` has not
advanced for more than 30 seconds. Prompt markers SHALL be matched
case-insensitively, as the compiled detectors match them, and the
result SHALL NOT depend on whether the markers were sourced from
the git-paw binary or from the helper's built-in fallback: the
profile stores markers in a normalised case, so a case-sensitive
match against the binary-sourced set silently misses prompts whose
on-screen text is capitalised differently.

#### Scenario: Permission prompt with stale heartbeat is detected

- **GIVEN** an agent whose pane shows `Do you want to
  proceed?` (or equivalent permission-prompt pattern) AND
  whose broker last_seen has not advanced for 45 seconds
- **WHEN** the next sweep iteration runs
- **THEN** the helper SHALL classify the pane as
  stuck-on-prompt

#### Scenario: Permission prompt with fresh heartbeat is not stuck

- **GIVEN** an agent whose pane shows a permission prompt AND
  whose last_seen is 5 seconds old
- **WHEN** the sweep runs
- **THEN** the helper SHALL NOT yet classify the pane as
  stuck (the heartbeat may have caught it pre-stall)

#### Scenario: Paste-buffer stall is detected

- **GIVEN** an agent whose pane shows `Pasted text #N` (the
  Claude paste-buffer indicator) AND whose last_seen is
  stale
- **WHEN** the sweep runs
- **THEN** the helper SHALL classify the pane as
  stuck-on-prompt with a `detail` annotation indicating the
  paste-buffer variant

#### Scenario: Binary-sourced markers detect a capitalised prompt

- **GIVEN** a git-paw binary resolvable on `PATH`, so the helper sources its prompt markers from it
- **AND** an agent whose pane shows `Do you want to proceed?` with a numbered option list and no other approval text, AND whose last_seen is stale
- **WHEN** the sweep runs
- **THEN** the helper SHALL classify the pane as stuck-on-prompt

#### Scenario: Marker source does not change the verdict

- **WHEN** the same capture is evaluated once with the binary-sourced markers and once with the helper's built-in fallback markers
- **THEN** both evaluations SHALL produce the same classification
