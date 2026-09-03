## 1. Spec Kit doctrine arm

- [ ] 1.1 Add a `SpecBackendKind::SpecKit` arm to `render_spec_path_doctrine` in `src/skills.rs` naming the Spec Kit spec location (`specs/<feature>/` or the injected sidecar), sourced from what the SpecKit backend already resolves
- [ ] 1.2 Grep `SpecBackendKind::SpecKit` across `src/` (per the variant-ripple checklist) to confirm no other consumer equates SpecKit with the OpenSpec path
- [ ] 1.3 Tests: the doctrine rendered for SpecKit names the Spec Kit location and does NOT contain `openspec/changes/`; the OpenSpec doctrine still names `openspec/changes/`

## 2. Gates

- [ ] 2.1 `just check` green
- [ ] 2.2 Every spec scenario maps to a test
