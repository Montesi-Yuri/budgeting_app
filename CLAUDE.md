# Budgeting App

Personal budgeting app for one user, PC (Windows) and Android phone, built with Tauri + React. The Devices sync directly with each other, with no cloud. Start with `README.md`, `CONTEXT.md` (glossary) and `docs/sync-design.md` (data model and sync protocol).

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature-slug>/issues/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`), recorded on each ticket's `Status:` line. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
