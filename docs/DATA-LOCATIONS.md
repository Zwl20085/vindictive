# Where Vindictive keeps things

| What | Location |
| --- | --- |
| Settings | `%APPDATA%\dev.zhangwentao.vindictive\settings.json` |
| Offline tip cache | `%APPDATA%\dev.zhangwentao.vindictive\tips-cache.json` |
| Fired reminders | `%APPDATA%\dev.zhangwentao.vindictive\fired.json` |
| Editor sessions | `%APPDATA%\dev.zhangwentao.vindictive\edits.json` (which files are watched and what is pending) |
| Local edit copies | `%APPDATA%\dev.zhangwentao.vindictive\edit\<repo path>` (written by *Edit locally*, pushed on every save) |
| Conflict copies | `<name>.remote.md` next to a local edit copy: GitHub's version when the tip changed there while you had unpushed edits. Merge into your copy and save; nothing is pushed until you do. |
| Log files | `%LOCALAPPDATA%\dev.zhangwentao.vindictive\logs\` |
| GitHub token | Windows Credential Manager, generic credential `vindictive` (user name `github-token`) |

The token is never written to any of the JSON files or the log.
