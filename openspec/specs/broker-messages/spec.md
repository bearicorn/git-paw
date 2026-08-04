# broker-messages Specification

## Purpose
Forward-compatible deserialization of broker message payloads for the v1.0 wire freeze: the three `Vec` payload fields with no non-empty contract default to an empty vector when absent (so lean and third-party publishers parse), while fields carrying an intentional non-empty contract stay required. Serialized output is unchanged — empty arrays are still emitted on the wire.

## Requirements
### Requirement: Lenient list-field deserialization

The broker message deserializer SHALL treat the three required `Vec` payload
fields that have no non-empty contract — `StatusPayload.modified_files`,
`ArtifactPayload.exports`, and `ArtifactPayload.modified_files` — as defaulting
to an empty vector when the field is absent from the input JSON, so that a
message that omits any of them still deserializes successfully instead of
failing with a hard parse error. This is a relax-only widening of accepted
input: serialization behaviour is unchanged and these fields SHALL continue to
be emitted (empty arrays are still serialized, so no existing wire scenario is
altered).

Fields that carry a non-empty contract — `IntentPayload.files` (validated by
`EmptyIntentFiles`) and `FeedbackPayload.errors` (validated by `EmptyErrors`) —
SHALL NOT be defaulted. They stay required, so omitting one remains a clean hard
missing-field parse error rather than a defaulted-then-validation error, and
their non-empty validators SHALL still reject an explicit empty array. This
keeps the frozen-surface change to the minimum necessary.

#### Scenario: Absent modified_files in a status payload defaults to empty

- **GIVEN** the JSON `{"status":"idle"}` with no `modified_files` key
- **WHEN** it is deserialized as `StatusPayload`
- **THEN** deserialization SHALL succeed
- **AND** `modified_files` SHALL be an empty vector

#### Scenario: Minimal status message parses via from_json

- **GIVEN** the JSON `{"type":"agent.status","agent_id":"feat-x","payload":{"status":"idle"}}` with no `modified_files` key in the payload
- **WHEN** it is parsed via `BrokerMessage::from_json`
- **THEN** parsing SHALL succeed and produce a `BrokerMessage::Status`
- **AND** the payload's `modified_files` SHALL be an empty vector

#### Scenario: Absent exports and modified_files in an artifact payload default to empty

- **GIVEN** the JSON `{"status":"done"}` with neither an `exports` nor a `modified_files` key
- **WHEN** it is deserialized as `ArtifactPayload`
- **THEN** deserialization SHALL succeed
- **AND** both `exports` and `modified_files` SHALL be empty vectors

#### Scenario: Serialization still emits the empty arrays

- **GIVEN** an `ArtifactPayload { status: "done", exports: vec![], modified_files: vec![] }`
- **WHEN** it is serialized to JSON
- **THEN** the JSON SHALL contain `exports` as an empty array `[]`
- **AND** the JSON SHALL contain `modified_files` as an empty array `[]`

#### Scenario: Non-empty-contract fields stay required and are not defaulted

- **GIVEN** the JSON `{"type":"agent.feedback","agent_id":"feat-x","payload":{"from":"supervisor"}}` with no `errors` key in the payload
- **WHEN** it is parsed via `BrokerMessage::from_json`
- **THEN** parsing SHALL fail with a hard missing-field deserialization error for `errors` (the field is required, NOT defaulted to an empty vector)
- **AND** supplying an explicit `"errors": []` SHALL still be rejected by the non-empty validator with the empty-errors error
- **AND** the same required-field behaviour SHALL hold for `IntentPayload.files`

#### Scenario: Existing populated messages round-trip unchanged

- **GIVEN** an `ArtifactPayload` with a non-empty `exports` and `modified_files`
- **WHEN** it is serialized and then deserialized
- **THEN** the round-trip SHALL preserve the value byte-equivalently, unchanged from the pre-freeze behaviour

