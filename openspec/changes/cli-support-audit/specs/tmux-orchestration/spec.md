## ADDED Requirements

### Requirement: The launch gate dismisses a provider's notice and stops at a blocking dialog

The launch-readiness gate SHALL recognise two kinds of start-up dialog, besides acceptance and
folder-trust dialogs, from the provider record of the CLI being launched, and SHALL handle them as
follows:

- A **notice**, for example a post-update "what's new" screen, that the CLI dismisses with a key the
  provider records. The gate SHALL send that key at most once per launch attempt, as a named key of
  the terminal port and never as text, and only after a screen read that recognised the notice. It
  SHALL then poll for readiness within the same budget. It SHALL NOT inject the boot block or any
  other text while the notice shows, and SHALL NOT relaunch the CLI into it. A notice still showing
  when the budget elapses after its key was sent SHALL be handled as an unresolved dialog.
- A **blocking** dialog that reports the CLI cannot serve, for example a usage-limit notice. The gate
  SHALL send it no key and no text. It SHALL handle the pane as blocked on an unresolved dialog, with
  an error that says the CLI reports it is unavailable and quotes the recognised line.

A recognised notice or blocking dialog SHALL take precedence over a readiness marker on the same
screen, because a dialog drawn over the CLI's idle screen can leave that screen's markers visible.
An unresolved dialog SHALL be handled as "Launch-readiness gate before boot-block injection"
specifies. Notices, blocking dialogs and a notice's key SHALL be compiled provider data. They SHALL
NOT be readable or overridable from a prompt-shape profile or a configuration file. A pane whose
CLI's provider declares neither kind SHALL be handled exactly as before.

#### Scenario: A notice is dismissed and the boot block follows

- **GIVEN** a pane whose CLI's provider declares a notice and its dismiss key, showing that notice
- **WHEN** the launch gate polls the pane and the CLI then shows its ready screen
- **THEN** the gate SHALL send the dismiss key exactly once
- **AND** no text SHALL be sent to the pane while the notice shows
- **AND** the boot block SHALL be injected once, after the ready screen is read

#### Scenario: A notice that does not clear fails the launch

- **GIVEN** a pane showing a provider-declared notice that is still showing after its key was sent
- **WHEN** the readiness budget elapses
- **THEN** the launch SHALL fail with an error naming the pane and the attach command
- **AND** the dismiss key SHALL have been sent exactly once and no boot block SHALL have been typed

#### Scenario: A notice drawn over the ready screen still comes first

- **GIVEN** a pane whose screen shows both a provider-declared notice and one of the CLI's readiness
  markers
- **WHEN** the launch gate polls the pane
- **THEN** the gate SHALL dismiss the notice before it injects anything

#### Scenario: A blocking dialog is never answered

- **GIVEN** a pane showing a provider-declared blocking dialog
- **WHEN** the launch gate polls the pane
- **THEN** no key and no text SHALL be sent to the pane
- **AND** the launch SHALL fail with an error that says the CLI reports it is unavailable and quotes
  the recognised line

#### Scenario: Another CLI's notice text is not acted on

- **GIVEN** a pane whose CLI's provider declares no notice, showing text identical to a notice
  another provider declares
- **WHEN** the launch gate polls the pane
- **THEN** no dismiss key SHALL be sent, and the pane SHALL be handled as it was before this
  requirement
