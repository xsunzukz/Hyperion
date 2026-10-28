# Contributors

| Contributor | Role |
| --- | --- |
| **xsunzukz** | Project author and maintainer — product direction, architecture, feature design, testing and releases |
| **[pranav158](https://github.com/pranav158)** | Contributor — release-readiness audit, install/verify/repair and download-speed fixes, encrypted Epic sign-in, Dauntless community build testing |
| **DeepSeek Harness** (AI coding agent) | Pair-programming assistant — implementation work on the launcher UI, the Steam sign-in flow, the manifest/CDN install pipeline and repository preparation |
| **Claude Code** (AI coding agent) | Pair-programming assistant to pranav158 — audit, then fixes: safe install/cancel/uninstall paths, chunk SHA-1 checks, Verify & Repair, library refresh and Epic sign-in renewal, antivirus-trigger removal, stall-proof hedged downloads, DPAPI-sealed Epic session |

## How the AI assistance was used

Parts of this repository were written collaboratively with **DeepSeek Harness**, an AI coding
agent, working inside the maintainer's editor under their direction. Every change was proposed by
the agent, reviewed and accepted by the maintainer, who retains authorship, ownership and
responsibility for the project.

The agent works under the following commit identity in this repository:

```
DeepSeek Harness <deepseek-harness@users.noreply.github.com>
```

Areas where the agent contributed in this repository:

- Collapsible sidebar and the "Running now" stop controls
- Steam sign-in unification (in-app credential modal instead of a web window)
- Manifest install flow and CDN resolution for manifest-only mirrors
- Header layout/compaction, download error reporting
- Repository preparation: `.gitignore`, `.gitattributes`, licence file, README

## Third-party attribution

Protocol research and manifest/chunk format details are based on the
[Legendary project](https://github.com/legendary-gl/legendary) by **derrod** (GPL-3.0-or-later).
Steam downloads are performed by Valve's headless *SteamCMD*, fetched and driven by the launcher.
