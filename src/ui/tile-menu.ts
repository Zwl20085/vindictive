/** Right-click menu on a tile: the detail actions without flipping. */
import type { Tip } from '../types';
import { t } from '../lib/i18n';
import { minutesUntilTomorrowMorning } from '../lib/time';
import { SNOOZE_HOUR_MINUTES, SWATCHES, type DetailActions } from './detail';
import { showMenu, showSwatchMenu, type MenuItem } from './menu';

export type TileMenuActions = Pick<
  DetailActions,
  'onDone' | 'onReopen' | 'onSnooze' | 'onSetColor' | 'onEdit' | 'onEditLocal' | 'onReveal' | 'onDelete'
>;

/** The menu entries for `tip`; sub-menus open at (`x`, `y`). */
export function tileMenuItems(tip: Tip, a: TileMenuActions, x: number, y: number, now = new Date()): MenuItem[] {
  const id = tip.id;
  const open = tip.status === 'open';
  return [
    open ? { label: t('done'), onSelect: () => a.onDone(id) } : { label: t('reopen'), onSelect: () => a.onReopen(id) },
    ...(open
      ? [
          { label: t('snooze1h'), onSelect: () => a.onSnooze(id, SNOOZE_HOUR_MINUTES) },
          { label: t('tomorrow'), onSelect: () => a.onSnooze(id, minutesUntilTomorrowMorning(now)) },
        ]
      : []),
    {
      label: `${t('colour')}…`,
      onSelect: () =>
        showSwatchMenu(x, y, {
          swatches: SWATCHES,
          current: tip.color,
          autoLabel: t('auto'),
          customLabel: t('customColour'),
          onPick: (color) => a.onSetColor(id, color),
        }),
    },
    { label: `${t('edit')}…`, onSelect: () => a.onEdit(id) },
    { label: t('openInEditor'), onSelect: () => a.onEditLocal(id) },
    { label: t('showInFolder'), onSelect: () => a.onReveal(id) },
    {
      label: `${t('delete')}…`,
      onSelect: () =>
        showMenu(x, y, [
          { label: `${t('deleteTip')}: ${tip.title}`, onSelect: () => a.onDelete(id) },
          { label: t('cancel'), onSelect: () => undefined },
        ]),
    },
  ];
}
