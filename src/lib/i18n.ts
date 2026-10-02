/**
 * Tiny string table for the two UI languages. `setLocale` is called whenever
 * settings arrive; every renderer reads through `t()` so a language switch
 * takes effect on the next render.
 */
import type { Kind, Language } from '../types';

export const LOCALES: readonly Language[] = ['en', 'zh'];

const STRINGS = {
  en: {
    appName: 'vindictive',
    next: 'NEXT',
    overdue: 'overdue',
    kind_task: 'Task',
    kind_deadline: 'Deadline',
    kind_note: 'Note',
    kind_reading: 'Reading',
    kind_event: 'Event',
    nextUp: 'Next up',
    emptyReady: 'Nothing to do. Add a tip with the hotkey, or drop a Markdown file into the tips folder.',
    emptyNoFolder: 'Tips live as Markdown files in a folder you choose. Put it in OneDrive to sync between PCs.',
    chooseFolder: 'Choose tips folder',
    tipsFolder: 'Tips folder',
    folderHint: 'Pick a folder inside OneDrive and every PC signed in to the same OneDrive shares the same tips. Only you can see them.',
    folderRequired: 'choose a tips folder',
    browse: 'Browse',
    openFolder: 'Open tips folder',
    showInFolder: 'Show in folder',
    syncing: 'Syncing…',
    syncError: 'Sync error',
    synced: 'Synced',
    notSynced: 'Not synced yet',
    menuSync: 'Reload tips folder',
    menuSettings: 'Settings',
    menuShowDone: 'Show done',
    menuDockLeft: 'Dock left',
    menuDockRight: 'Dock right',
    menuFree: 'Free',
    menuQuit: 'Quit',
    where: 'where',
    repeat: 'repeat',
    tags: 'tags',
    snoozed: 'snoozed',
    figures: 'figures',
    paperNotFetched: 'metadata not fetched yet',
    done: 'Done',
    reopen: 'Reopen',
    snooze1h: 'Snooze 1h',
    tomorrow: 'Tomorrow',
    back: 'Back',
    close: 'Close',
    settings: 'Settings',
    hotkey: 'Capture hotkey',
    boardHotkey: 'Peek hotkey (blank = off)',
    hotkeysClash: 'the peek hotkey must be different from the capture hotkey',
    autoUpdateCheck: 'Check for updates in the background',
    dock: 'Dock',
    theme: 'Theme',
    language: 'Language',
    columns: 'Columns',
    weatherLocation: 'Weather city (blank = off)',
    showPanel: 'Show clock and weather panel',
    alwaysOnTop: 'Always on top',
    alwaysOnBottom: 'Always on bottom',
    layerConflict: 'always on top and always on bottom cannot both be on',
    autostart: 'Start with Windows',
    notifyNew: 'Toast when a new tip arrives',
    showDoneTiles: 'Show done tiles',
    save: 'Save',
    saved: 'Saved',
    weatherOff: 'weather off',
    weatherLoading: 'weather…',
    weatherUnavailable: 'weather unavailable',
    high: 'H',
    low: 'L',
    captureHint: '#tag  !high/!low  @today @tomorrow @2026-10-15 09:00  ^"Lab 302"  >note  arXiv/DOI',
    capturePlaceholder: 'What next?',
    captureSaved: 'Saved',
    dark: 'dark',
    light: 'light',
    nerv: 'NERV',
    cobalt: 'cobalt',
    paper: 'paper',
    delete: 'Delete',
    deleteConfirm: 'Delete?',
    deleteTip: 'Delete tip',
    cancel: 'Cancel',
    addTip: 'New tip',
    edit: 'Edit',
    openInEditor: 'Open in editor',
    editTip: 'Edit tip',
    fieldTitle: 'Title',
    fieldKind: 'Kind',
    fieldPriority: 'Priority',
    prio_low: 'Low',
    prio_normal: 'Normal',
    prio_high: 'High',
    fieldDueDate: 'Due date (blank = none)',
    fieldDueTime: 'Time (optional)',
    fieldLocation: 'Location',
    fieldTags: 'Tags (space or comma separated)',
    fieldBody: 'Notes (Markdown)',
    editHint: 'Ctrl+Enter saves · Esc cancels',
    titleRequired: 'a tip needs a title',
    timeNeedsDate: 'pick a date for that time',
    tooLong: 'is too long',
    badChoice: 'unknown',
    editorCommand: 'Editor command (blank = default app)',
    openedIn: 'Opened',
    fitHeight: 'Window height follows the tiles',
    windowOpacity: 'Window opacity % (backgrounds only)',
    customColour: 'Pick any colour',
    tile: 'Tile',
    size: 'Size',
    colour: 'Colour',
    auto: 'Auto',
    sizeSm: 'Small',
    sizeMd: 'Medium',
    sizeWide: 'Wide',
    en: 'English',
    zh: '中文',
    left: 'left',
    right: 'right',
    free: 'free',
    clawdCelebrating: 'one down!',
    clawdSleeping: 'zzz',
    clawdWorried: '{n} overdue',
    clawdRelaxed: 'all done',
    clawdWalking: '{n} to do',
  },
  zh: {
    appName: '复仇者',
    next: '下一项',
    overdue: '已逾期',
    kind_task: '任务',
    kind_deadline: '截止',
    kind_note: '笔记',
    kind_reading: '阅读',
    kind_event: '日程',
    nextUp: '下一项',
    emptyReady: '暂无事项。用快捷键新增，或把 Markdown 文件放进事项文件夹。',
    emptyNoFolder: '事项以 Markdown 文件保存在你选择的文件夹中。放在 OneDrive 里即可在多台电脑间同步。',
    chooseFolder: '选择事项文件夹',
    tipsFolder: '事项文件夹',
    folderHint: '选择 OneDrive 中的文件夹，登录同一 OneDrive 的每台电脑都会看到同样的事项，且只有你自己可见。',
    folderRequired: '请选择事项文件夹',
    browse: '浏览',
    openFolder: '打开事项文件夹',
    showInFolder: '在文件夹中显示',
    syncing: '同步中…',
    syncError: '同步错误',
    synced: '已同步',
    notSynced: '尚未同步',
    menuSync: '重新读取事项文件夹',
    menuSettings: '设置',
    menuShowDone: '显示已完成',
    menuDockLeft: '停靠左侧',
    menuDockRight: '停靠右侧',
    menuFree: '自由位置',
    menuQuit: '退出',
    where: '地点',
    repeat: '重复',
    tags: '标签',
    snoozed: '推迟至',
    figures: '图表',
    paperNotFetched: '尚未获取文献信息',
    done: '完成',
    reopen: '重新打开',
    snooze1h: '推迟 1 小时',
    tomorrow: '明天',
    back: '返回',
    close: '关闭',
    settings: '设置',
    hotkey: '快速记录快捷键',
    boardHotkey: '查看面板快捷键（留空关闭）',
    hotkeysClash: '查看面板快捷键必须与快速记录快捷键不同',
    autoUpdateCheck: '在后台检查更新',
    dock: '停靠',
    theme: '主题',
    language: '语言',
    columns: '列数',
    weatherLocation: '天气城市（留空关闭）',
    showPanel: '显示时钟与天气面板',
    alwaysOnTop: '窗口置顶',
    alwaysOnBottom: '窗口置底',
    layerConflict: '窗口置顶与窗口置底不能同时开启',
    autostart: '开机启动',
    notifyNew: '新事项到达时弹出通知',
    showDoneTiles: '显示已完成磁贴',
    save: '保存',
    saved: '已保存',
    weatherOff: '天气未开启',
    weatherLoading: '获取天气…',
    weatherUnavailable: '天气不可用',
    high: '高',
    low: '低',
    captureHint: '#标签  !high/!low  @today @tomorrow @2026-10-15 09:00  ^"实验室302"  >note  arXiv/DOI',
    capturePlaceholder: '接下来做什么？',
    captureSaved: '已保存',
    dark: '深色',
    light: '浅色',
    nerv: 'NERV',
    cobalt: '钴蓝',
    paper: '纸白',
    delete: '删除',
    deleteConfirm: '确认删除？',
    deleteTip: '删除事项',
    cancel: '取消',
    addTip: '新事项',
    edit: '编辑',
    openInEditor: '在编辑器中打开',
    editTip: '编辑事项',
    fieldTitle: '标题',
    fieldKind: '类型',
    fieldPriority: '优先级',
    prio_low: '低',
    prio_normal: '普通',
    prio_high: '高',
    fieldDueDate: '截止日期（留空表示无）',
    fieldDueTime: '时间（可选）',
    fieldLocation: '地点',
    fieldTags: '标签（空格或逗号分隔）',
    fieldBody: '正文（Markdown）',
    editHint: 'Ctrl+Enter 保存 · Esc 取消',
    titleRequired: '请填写标题',
    timeNeedsDate: '请先为该时间选择日期',
    tooLong: '太长',
    badChoice: '未知的',
    editorCommand: '编辑器命令（留空用默认程序）',
    openedIn: '已打开',
    fitHeight: '窗口高度随磁贴自适应',
    windowOpacity: '窗口透明度 %（仅背景）',
    customColour: '自定义颜色',
    tile: '磁贴',
    size: '大小',
    colour: '颜色',
    auto: '自动',
    sizeSm: '小',
    sizeMd: '中',
    sizeWide: '宽',
    en: 'English',
    zh: '中文',
    left: '左',
    right: '右',
    free: '自由',
    clawdCelebrating: '完成一项！',
    clawdSleeping: 'zzz',
    clawdWorried: '{n} 项逾期',
    clawdRelaxed: '全部完成',
    clawdWalking: '{n} 项待办',
  },
} as const;

export type StringKey = keyof (typeof STRINGS)['en'];

let current: Language = 'en';

export function isLanguage(value: unknown): value is Language {
  return typeof value === 'string' && (LOCALES as readonly string[]).includes(value);
}

export function setLocale(lang: Language | undefined): void {
  current = isLanguage(lang) ? lang : 'en';
  if (typeof document !== 'undefined') {
    document.documentElement.lang = current === 'zh' ? 'zh-CN' : 'en';
  }
}

export function locale(): Language {
  return current;
}

/** Look up a UI string in the active language, falling back to English. */
export function t(key: StringKey, lang: Language = current): string {
  return STRINGS[lang][key] ?? STRINGS.en[key];
}

export function kindLabel(kind: Kind, lang: Language = current): string {
  return t(`kind_${kind}` as StringKey, lang);
}

const WEEKDAYS = {
  en: ['SUN', 'MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT'],
  zh: ['周日', '周一', '周二', '周三', '周四', '周五', '周六'],
} as const;

const MONTHS_EN = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

export function weekday(date: Date, lang: Language = current): string {
  return WEEKDAYS[lang][date.getDay()] ?? '';
}

/** Panel date line: `2026-09-28 MON` or `2026年9月28日 周一`. */
export function formatPanelDate(date: Date, lang: Language = current): string {
  const y = date.getFullYear();
  const m = date.getMonth() + 1;
  const d = date.getDate();
  if (lang === 'zh') return `${y}年${m}月${d}日 ${weekday(date, lang)}`;
  const pad = (n: number): string => String(n).padStart(2, '0');
  return `${y}-${pad(m)}-${pad(d)} ${weekday(date, lang)}`;
}

/** Human date for the detail view: `Oct 15, 23:59` or `10月15日 23:59`. */
export function formatDateTime(date: Date, lang: Language = current): string {
  const pad = (n: number): string => String(n).padStart(2, '0');
  const hm = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  if (lang === 'zh') return `${date.getMonth() + 1}月${date.getDate()}日 ${hm}`;
  return `${MONTHS_EN[date.getMonth()]} ${date.getDate()}, ${hm}`;
}

/** Duration units for countdowns. */
export function spanUnits(lang: Language = current): { w: string; d: string; h: string; m: string; sep: string } {
  return lang === 'zh' ? { w: '周', d: '天', h: '时', m: '分', sep: '' } : { w: 'w', d: 'd', h: 'h', m: 'm', sep: ' ' };
}
