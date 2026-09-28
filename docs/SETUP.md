# Setup

## 1. Create the tips repository

1. On GitHub, **New repository** → name it `vindictive-tips` (any name works)
   → **Private** → Create.
2. Add a `tips/` folder. The quickest way is to copy
   [`examples/tips/`](../examples/tips) from this repo, or create one file:

   ```markdown
   ---
   title: Hello
   ---
   ```

   saved as `tips/hello.md`.

## 2. Create a fine-grained token

The app needs to read and write files in that one repository, nothing else.

1. GitHub → your avatar → **Settings**.
2. Left sidebar, bottom → **Developer settings**.
3. **Personal access tokens** → **Fine-grained tokens** → **Generate new token**.
4. Token name: `vindictive`. Expiration: your choice (1 year is fine; you will
   get an email before it expires).
5. **Repository access** → *Only select repositories* → pick `vindictive-tips`.
6. **Permissions** → **Repository permissions**:
   - **Contents**: *Read and write*
   - **Metadata**: *Read-only* (selected automatically)
7. **Generate token** and copy it. It starts with `github_pat_`.

## 3. Connect the app

1. Start Vindictive. Right-click the thin strip at the top of the board →
   **Settings**.
2. Fill in owner (`Zwl20085`), repo (`vindictive-tips`), branch (`main`),
   dir (`tips`).
3. Paste the token in the token field and press **Save**.
4. Press **Test connection**. You should see `OK: n tips in owner/repo/tips`.

The token goes to **Windows Credential Manager** under the generic credential
`vindictive` (user name `github`). It is never written to a settings file, the
tips repo, or a log. To remove it: Control Panel → Credential Manager →
Windows Credentials → `vindictive` → Remove, or press **Forget token** in
Settings.

## 4. Optional

- **Start with Windows**: Settings → *Autostart*.
- **Dock**: right-click the strip → *Dock left* / *Dock right* / *Free*.
- **Hotkey**: default `Ctrl+Shift+Space`, change it in Settings.
- **Poll interval**: default 60 s. The app uses conditional requests, so an
  unchanged repo costs one request per poll.

## Troubleshooting

| Symptom | Cause and fix |
| ------- | ------------- |
| `401 Unauthorized` on Test connection | Token expired, was pasted with a trailing space, or is a classic token without `repo` scope. Generate a fine-grained token as above. |
| `404 Not Found` | The `dir` does not exist on that branch, the repo name is wrong, or the token was not granted access to this repository (step 5). |
| `403 rate limit exceeded` | You are polling faster than GitHub allows (5 000 requests/hour). Raise *Poll interval*; 60 s is safe. |
| Tips appear but images do not | Image path must be relative to `dir`, and the file must be under 1 MB. |
| No toasts | Windows *Focus Assist* / *Do not disturb* is on, or notifications for Vindictive are off in *Settings → System → Notifications*. |
| Toasts arrive but the board is not on top | Another window with the same flag was focused later. Click the board once; use *Dock* to keep it at an edge. |
| Board vanished | It is in the tray. Click the tray icon → *Show*. |
| A tip I edited on GitHub does not update | Wait one poll interval or right-click the strip → *Sync now*. Files that do not start with `---` are ignored; check the log in `%LOCALAPPDATA%\dev.zhangwentao.vindictive\logs` (see [DATA-LOCATIONS.md](DATA-LOCATIONS.md)). |
