# Where Vindictive keeps things

`%APPDATA%` is the roaming profile (`C:\Users\<you>\AppData\Roaming`), which
is where Tauri's `app_data_dir()` points on Windows; `%LOCALAPPDATA%` is
`AppData\Local`. The JSON file names are defined in
`src-tauri/src/app/storage.rs`.

| What | Location |
| --- | --- |
| Settings | `%APPDATA%\dev.zhangwentao.vindictive\settings.json` |
| Offline tip cache | `%APPDATA%\dev.zhangwentao.vindictive\tips-cache.json` |
| Fired reminders | `%APPDATA%\dev.zhangwentao.vindictive\fired.json` |
| Log files | `%LOCALAPPDATA%\dev.zhangwentao.vindictive\logs\` |
| WebView2 data (browser cache) | `%LOCALAPPDATA%\dev.zhangwentao.vindictive\EBWebView\` |
| Tips | Wherever you set the tips folder (default: `%OneDrive%\Vindictive` or `Documents\Vindictive`) |

Images referenced in tips are files in the same tips folder or subfolders
(e.g., `figures/plot.png` → `%OneDrive%\Vindictive\figures\plot.png`).

Copies upgraded from the GitHub-backed 0.2.x versions may also have an
`edit\` folder and an `edits.json` file in `%APPDATA%\dev.zhangwentao.vindictive`.
Current versions do not use them; they are safe to delete.
