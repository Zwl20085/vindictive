# Setup

Vindictive is set up automatically on first launch. No accounts, tokens, or
configuration needed.

## Automatic setup

When you run Vindictive the first time, it picks a tips folder:

- **If OneDrive is installed:** `%OneDrive%\Vindictive`
- **Otherwise:** `Documents\Vindictive` in your user's home

The folder is created if it doesn't exist. Tips are synced between PCs through
OneDrive.

Press `Ctrl+Shift+Space` and type your first tip. That's it.

## Using a different folder

If you want tips in a different location (another sync tool, a NAS share, etc.):

1. Open Settings (right-click the board strip → **Settings**)
2. Under **Tips folder**, click **Browse…** and pick the folder
3. Click **Save**

The folder is created if needed and the board reloads from the new folder.
Nothing is copied or moved: tips in the old folder stay where they are. To
take them with you, copy the `.md` files (and any `figures/`) into the new
folder yourself, before or after switching.

Settings → **Highlight words**: words or phrases (one per entry) marked on every tile and tip body, ignoring case, on top of each tip's own `highlight:` list.

## Hotkeys

| Hotkey | Default | Does |
| ------ | ------- | ---- |
| Capture | `Ctrl+Shift+Space` | opens the one-line quick-capture bar |
| Peek | `Ctrl+Alt+Shift+Space` | lifts the board above other windows; press again, click the tray icon or switch to another app to drop it back (dragging or resizing the lifted board keeps it up) |

Both are global and can be rebound in Settings (**Capture hotkey**, **Peek
hotkey**). Type a combination such as `Ctrl+Alt+N` and **Save**. The capture
hotkey is required; leave the peek hotkey blank to turn it off. The two must
differ, and a combination that does not parse is rejected when you save.

## Syncing between PCs

Each PC just needs to be signed into the same OneDrive account. Changes appear
on all PCs within a few seconds as each one rescans the tips folder. Any sync
tool works: Dropbox, Syncthing, a NAS share, or a plain USB drive you carry.
To rescan at once, click the small square at the left of the board strip
(green when the last scan was fine, red on an error).

## Migrating from the GitHub version (0.2.x)

If you were using Vindictive 0.2.x with GitHub:

1. In your old repo, download or copy the `tips/` folder (including `figures/`
   if you have images)
2. Paste it into the new tips folder (either auto-created or one you picked in
   Settings)
3. The old settings are ignored harmlessly. You can remove the fine-grained
   token from Windows Credential Manager: **Control Panel** →
   **Credential Manager** → **Windows Credentials** → `vindictive` →
   **Remove**
4. The old repo can be archived or deleted

## Uninstall

1. Optional: in Settings, untick **Start with Windows** first. This removes
   the autostart entry (a `Vindictive` value under
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). If you skip
   this, disable or remove it afterwards in **Task Manager → Startup apps**.
2. **Settings → Apps → Installed apps → Vindictive → Uninstall**. The
   `setup.exe` installer is per-user, so no admin rights are needed.
3. App data is kept unless you tick **Delete the application data** in the
   uninstaller (the `setup.exe` install only; the `.msi` has no such option).
   To remove it by hand, delete `%APPDATA%\dev.zhangwentao.vindictive`
   (settings, tip cache, fired reminders) and
   `%LOCALAPPDATA%\dev.zhangwentao.vindictive` (logs, WebView data). See
   [DATA-LOCATIONS.md](DATA-LOCATIONS.md).
4. Your tips folder is never touched. Delete it yourself if you no longer
   want the tips.

## Building from source

Vindictive is built, tested and released for Windows only (Windows 10 21H2
or later, WebView2). CI runs on `windows-latest` only, and Linux and macOS
builds are untested; they are on the roadmap, not supported today. You need Rust stable, the MSVC C++ build
tools, and Node 20 or later; see the README's *Build from source* section.

## Troubleshooting

| Symptom | Cause and fix |
| ------- | ------------- |
| Board is empty on first launch | The tips folder was just created. Press `Ctrl+Shift+Space` to create your first tip. |
| I want to move my tips folder | Copy the files from the old folder to the new one in Explorer, then open Settings, click **Browse…** next to **Tips folder**, pick the new location, and **Save**. The app does not copy or move files itself. |
| Tips are not updating from my other PC | Check that both PCs are synced through the same tool (OneDrive, Dropbox, etc.) and that the tips folder path is the same or points to the same shared location. Wait up to 10 seconds for a rescan. |
| A tip I edited in my editor doesn't appear | The editor might not have written the file yet. Save explicitly if the editor shows unsaved changes. Check the log at `%LOCALAPPDATA%\dev.zhangwentao.vindictive\logs\` for any parsing errors. |
| No toasts | Windows *Focus Assist* / *Do not disturb* is on, or notifications for Vindictive are off in *Settings → System → Notifications*. |
