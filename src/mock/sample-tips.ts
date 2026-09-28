import type { Settings, Tip } from '../types';
import { DAY_MS, HOUR_MS, toNaive } from '../lib/time';

const MONDAY = 1;

function nextMonday(now: Date, hour: number): Date {
  const daysAhead = ((MONDAY - now.getDay() + 7) % 7) || 7;
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + daysAhead, hour, 0, 0);
}

function at(now: Date, offsetMs: number): string {
  return toNaive(new Date(now.getTime() + offsetMs));
}

export const SAMPLE_SETTINGS: Settings = {
  owner: 'Zwl20085',
  repo: 'vindictive-tips',
  branch: 'main',
  dir: 'tips',
  poll_seconds: 60,
  hotkey: 'Ctrl+Shift+Space',
  dock: 'right',
  always_on_top: true,
  autostart: false,
  notify_new_tips: true,
  theme: 'dark',
  columns: 4,
  show_done: false,
};

/** Realistic researcher sample data for browser development. */
export function sampleTips(now: Date): Tip[] {
  const meeting = nextMonday(now, 10);
  return [
    {
      id: '2026-10-iemdc-abstract',
      path: 'tips/2026-10-iemdc-abstract.md',
      title: 'IEMDC 2027 abstract',
      kind: 'deadline',
      priority: 'high',
      status: 'open',
      due: at(now, 5 * DAY_MS),
      due_at: at(now, 5 * DAY_MS),
      remind: ['-1d'],
      remind_at: [at(now, 4 * DAY_MS)],
      links: ['https://www.iemdc.org/'],
      tags: ['iemdc', 'paper'],
      body: '## Abstract checklist\n\n- [x] 250 words\n- [ ] Compare with the 12-slot baseline\n- [ ] Ask co-authors for ORCID\n\nSubmit through the **EDAS** portal.',
    },
    {
      id: '2026-10-tie-revision',
      path: 'tips/2026-10-tie-revision.md',
      title: 'Upload TIE revision + response letter',
      kind: 'deadline',
      priority: 'normal',
      status: 'open',
      due_at: at(now, 30 * HOUR_MS),
      remind_at: [at(now, 6 * HOUR_MS)],
      links: ['https://mc.manuscriptcentral.com/tie'],
      tags: ['tie', 'revision'],
      body: 'Reviewer 2 wants the thermal model validated at 12 kHz.\n\n![thermal](figures/coil-thermal.png)',
      images: ['figures/coil-thermal.png'],
    },
    {
      id: 'group-meeting',
      path: 'tips/group-meeting.md',
      title: 'Group meeting',
      kind: 'event',
      priority: 'normal',
      status: 'open',
      due_at: toNaive(meeting),
      remind_at: [toNaive(new Date(meeting.getTime() - 30 * 60_000))],
      repeat: 'weekly on mon at 10:00',
      location: 'Room 4.12',
      tags: ['group'],
      body: 'Bring the loss-map slides.',
    },
    {
      id: 'read-2401-12345',
      path: 'tips/read-2401-12345.md',
      title: 'Read hairpin AC-loss paper',
      kind: 'reading',
      priority: 'normal',
      status: 'open',
      arxiv: '2401.12345',
      paper: {
        title: 'AC Loss Modelling of Hairpin Windings at High Switching Frequency',
        authors: ['A. Example', 'B. Author', 'C. Person'],
        year: 2024,
        venue: 'arXiv',
        url: 'https://arxiv.org/abs/2401.12345',
      },
      remind_at: [],
      tags: ['reading'],
      body: 'Check whether their proximity-effect factor matches ours in section IV.',
    },
    {
      id: 'lab-dyno-coolant',
      path: 'tips/lab-dyno-coolant.md',
      title: 'Check dyno coolant level before the 8 kW run',
      kind: 'task',
      priority: 'high',
      status: 'open',
      location: 'Lab 302, bench 4',
      remind_at: [at(now, 2 * HOUR_MS)],
      tags: ['lab'],
      body: 'Top up with the 50/50 glycol mix. Log the level in the bench notebook.',
    },
    {
      id: 'note-winding-factor',
      path: 'tips/note-winding-factor.md',
      title: 'Winding factor derivation for 6-layer hairpin',
      kind: 'note',
      priority: 'low',
      status: 'open',
      remind_at: [],
      tags: ['theory'],
      images: ['figures/slot-star.png'],
      body: '- [ ] Redo the star-of-slots for q = 2\n- [ ] Cross-check with the FE result\n\n![star](figures/slot-star.png)',
    },
    {
      id: 'order-thermocouples',
      path: 'tips/order-thermocouples.md',
      title: 'Order 10× type-K thermocouples',
      kind: 'task',
      priority: 'low',
      status: 'open',
      remind_at: [],
      links: ['https://www.omega.com/'],
      body: '1 mm sheath, 2 m leads.',
    },
    {
      id: 'reply-reviewer-2',
      path: 'tips/reply-reviewer-2.md',
      title: 'Reply to Reviewer 2 e-mail',
      kind: 'task',
      priority: 'high',
      status: 'open',
      due_at: at(now, -DAY_MS),
      remind_at: [],
      body: 'Be polite. Attach the updated Fig. 4.',
    },
    {
      id: 'done-calibrate-scope',
      path: 'tips/done-calibrate-scope.md',
      title: 'Calibrate the oscilloscope probes',
      kind: 'task',
      priority: 'normal',
      status: 'done',
      done_at: at(now, -2 * DAY_MS),
      remind_at: [],
      body: 'Done with the 1 kHz reference.',
    },
  ];
}
