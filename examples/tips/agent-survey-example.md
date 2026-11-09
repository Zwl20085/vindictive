---
title: Survey hairpin AC loss papers
kind: agent
priority: normal
status: open
tags: [literature, ac-loss]
created: 2026-11-08 21:40
agent:
  v: 1
  state: done
  hash: sha256:d7e5fb1c7dc4e778c23524b52f0aa6da2953d0662e5dfe69cfab701a78df7476
  host: DESKTOP-ABC
  run: 20261109-193100-ab12
  at: 2026-11-09T19:31:00
  results: agent/agent-survey-example/
---

Survey recent papers on AC copper loss in hairpin windings of traction motors.
For each paper give the loss model, the frequency range and how it was validated.

<!-- agent:begin 2026-11-09T07:31:00 question -->
## ❓ Questions · 2026-11-09 07:31
1. Which year range should the survey cover?
2. IEEE only, or arXiv too?
<!-- agent:end -->

2020 onwards, and include arXiv.

<!-- agent:begin 2026-11-09T19:31:00 done -->
## ⭐ Result · 2026-11-09 19:31
Found 14 papers; the summary table is in `survey.md`.

Most use a 2D FE model with a strand-level mesh; four validate against a
locked-rotor test and two against a hairpin sample on an impedance analyser.

Files: agent/agent-survey-example/
<!-- agent:end -->
