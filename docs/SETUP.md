# Setup

The short version: on first launch click **Set up GitHub sync**, then use the
two buttons at the top of the GitHub section, **Create repo ↗** and
**Create token ↗**. Both open GitHub pages that are already filled in. The
long version, step by step:

## 1. Create the tips repository

1. In Vindictive Settings press **Create repo ↗**, or on GitHub go to
   **New repository**. Name it `vindictive-tips` (any name works), choose
   **Private**, tick **Add a README file**, and press **Create repository**.
   The README gives the repo a `main` branch; a repo with no commits at all
   has no branch to sync.
2. You don't need to create the `tips/` folder. Your first tip creates it.
   If you want a starter set, copy [`examples/tips/`](../examples/tips) from
   this repo into the tips repo.

## 2. Create a fine-grained token

The app needs to read and write files in that one repository, nothing else.

1. In Vindictive Settings press **Create token ↗**. GitHub opens
   *New fine-grained personal access token* with the name, a one-year expiry
   and **Contents: Read and write** already filled in.
   To do it by hand instead: avatar → **Settings** → **Developer settings** →
   **Personal access tokens** → **Fine-grained tokens** → **Generate new token**,
   then under **Permissions** set **Contents** to *Read and write*.
2. **Repository access** → *Only select repositories* → pick
   `vindictive-tips`. GitHub can't pre-fill this part.
3. **Generate token** and copy it. It starts with `github_pat_`.

## 3. Connect the app

1. Open Settings: click **Set up GitHub sync** on the empty board, or
   right-click the thin strip at the top of the board → **Settings**.
2. Paste the repository URL, e.g. `https://github.com/<you>/vindictive-tips`,
   into **Owner**. On Save it is split into owner and repo. You can also type
   them separately. Branch defaults to `main` and directory to `tips`.
3. Paste the token in the token field and press **Save token**, then **Save**.
4. Press **Test connection**. You should see `OK: n tips in <you>/vindictive-tips/tips`,
   or, for a fresh repo, a message that `tips/` will be created by your first tip.
5. Press `Ctrl+Shift+Space` anywhere and type your first tip.

The token goes to **Windows Credential Manager** under the generic credential
`vindictive` (user name `github-token`). It is never written to a settings
file, the tips repo, or a log. To remove it, press **Clear** next to the token
field, or go to Control Panel → Credential Manager → Windows Credentials →
`vindictive` → Remove.

## 4. Optional settings

In the same Settings overlay:

- **Language**: English or 中文. Applies to the board, the detail view, the
  settings and the capture bar.
- **Weather city**: a place name such as `Tokyo` or `南京`. The app looks it
  up on Open-Meteo (no account needed) and shows the current conditions in the
  panel above the tiles. Leave it blank to turn weather off.
- **Show clock and weather panel**: hides the whole panel when unticked.
- **Start with Windows**: the *Start with Windows* checkbox.
- **Always on top**: off by default; tick it to keep the board above every window.
- **Dock**: right-click the strip → *Dock left* / *Dock right* / *Free*.
- **Hotkey**: default `Ctrl+Shift+Space`.
- **Editor command**: what *Edit locally* runs; default `code` (VS Code),
  blank opens the file with the default app for `.md`.
- **Poll interval**: default 60 s. The app uses conditional requests, so an
  unchanged repo costs one request per poll.

## Troubleshooting

| Symptom | Cause and fix |
| ------- | ------------- |
| `401 Unauthorized` on Test connection | Token expired, was pasted with a trailing space, or is a classic token without `repo` scope. Generate a fine-grained token as above. |
| `404 Not Found` | The repo or branch name is wrong, the repo has no commits yet (add a README), or the token was not granted access to this repository (step 2.2). A missing `dir` is fine; it is created by your first tip. |
| `403 rate limit exceeded` | You are polling faster than GitHub allows (5 000 requests/hour). Raise *Poll interval*; 60 s is safe. |
| Tips appear but images do not | Image path must be relative to `dir`, and the file must be under 1 MB. |
| No toasts | Windows *Focus Assist* / *Do not disturb* is on, or notifications for Vindictive are off in *Settings → System → Notifications*. |
| Toasts arrive but the board is not on top | Another window with the same flag was focused later. Click the board once; use *Dock* to keep it at an edge. |
| Board vanished | It is in the tray. Click the tray icon → *Show*. |
| A tip I edited on GitHub does not update | Wait one poll interval or right-click the strip → *Sync now*. Files that do not start with `---` are ignored; check the log in `%LOCALAPPDATA%\dev.zhangwentao.vindictive\logs` (see [DATA-LOCATIONS.md](DATA-LOCATIONS.md)). |
