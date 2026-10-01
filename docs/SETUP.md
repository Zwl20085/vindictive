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

The folder is created if needed and the board reloads.

## Syncing between PCs

Each PC just needs to be signed into the same OneDrive account. Changes appear
on all PCs within a few seconds as each one rescans the tips folder. Any sync
tool works: Dropbox, Syncthing, a NAS share, or a plain USB drive you carry.

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

## Troubleshooting

| Symptom | Cause and fix |
| ------- | ------------- |
| Board is empty on first launch | The tips folder was just created. Press `Ctrl+Shift+Space` to create your first tip. |
| I want to move my tips folder | Open Settings, click **Browse…** next to **Tips folder**, pick the new location, and **Save**. The app copies/moves files as needed. |
| Tips are not updating from my other PC | Check that both PCs are synced through the same tool (OneDrive, Dropbox, etc.) and that the tips folder path is the same or points to the same shared location. Wait up to 10 seconds for a rescan. |
| A tip I edited in my editor doesn't appear | The editor might not have written the file yet. Save explicitly if the editor shows unsaved changes. Check the log at `%LOCALAPPDATA%\dev.zhangwentao.vindictive\logs\` for any parsing errors. |
| No toasts | Windows *Focus Assist* / *Do not disturb* is on, or notifications for Vindictive are off in *Settings → System → Notifications*. |
