# Examples

`tips/` is a ready-made starter set for your private tips repository.

1. Create a **private** repository on GitHub, for example `vindictive-tips`.
2. Copy the `tips/` folder from here into its root.
3. Point Vindictive at it: *Settings → owner / repo / branch / dir* with
   `dir = tips`.

The eight files cover every kind of tip the app understands:

| File | Shows |
| ---- | ----- |
| `2026-10-04-iemdc-abstract.md` | critical deadline, relative reminders, checklist |
| `2026-11-15-ecce-camera-ready.md` | later deadline, `-1w -3d -1d` remind list |
| `group-meeting.md` | recurring event that rolls forward on **Done** |
| `check-coil-temperature.md` | lab task with location and an image |
| `read-arxiv-2401-12345.md` | reading tip with a filled `paper` block |
| `read-doi-tie.md` | reading tip with a DOI only, metadata fetched later |
| `rebuttal-notes.md` | plain note with a task list body |
| `backup-nas.md` | a done task with `done_at` |

Dates in the examples are in autumn 2026; edit them so the deadlines are in
your future, otherwise everything shows as overdue.

Format reference: [`docs/TIP-FORMAT.md`](../docs/TIP-FORMAT.md).
