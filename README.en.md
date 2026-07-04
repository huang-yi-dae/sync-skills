<p align="center">
  <a href="README.md">中文</a> | <strong>English</strong> | <a href="README.ja.md">日本語</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.1.0-blue?style=flat-square" alt="version">
  <img src="https://img.shields.io/badge/Tauri-v2-orange?style=flat-square&logo=tauri" alt="tauri">
  <img src="https://img.shields.io/badge/Rust-2021-brown?style=flat-square&logo=rust" alt="rust">
  <img src="https://img.shields.io/badge/React-19-blue?style=flat-square&logo=react" alt="react">
  <img src="https://img.shields.io/badge/license-AGPL--3.0-green?style=flat-square" alt="license">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey?style=flat-square" alt="platform">
</p>

<h1 align="center">⬡ Skill Manager</h1>

<p align="center">
  <strong>Cross-tool Skill synchronization for AI coding assistants</strong><br>
  Edit once, sync everywhere.
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#installation">Installation</a> •
  <a href="#development">Development</a> •
  <a href="#roadmap">Roadmap</a>
</p>

---

## Why Skill Manager

AI coding assistants (Claude Code, Cursor, Windsurf, Cline, etc.) all use `SKILL.md` files to define reusable knowledge and workflows. When you use multiple tools, skill files scatter across different directories — manual syncing is tedious and error-prone.

Skill Manager provides a desktop GUI to manage all your skills in one place and automatically sync them across every configured AI tool.

## Features

- **Tool Management** — Register AI coding tool paths with auto-discovery for 13+ known tools
- **Skill Scanning** — Recursively scan directories to discover all `SKILL.md`-based skill directories
- **SSOT Sync** — Hub-and-spoke model centered on `~/.agents/skills/local/`, distributing to all tools
- **Project-level Management** — Configure independent skill sets per project
- **Diff Detection** — Built-in LCS unified diff view showing precise file-level changes
- **Theme Switching** — Light / Dark / Follow System
- **i18n** — 中文 / English
- **Activity Logs** — Complete audit trail of all operations

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Claude Code │     │   Cursor    │     │  Windsurf   │
│  .claude/    │     │  .cursor/   │     │  .windsurf/ │
└──────┬───────┘     └──────┬──────┘     └──────┬──────┘
       │                    │                    │
       │     Skill Manager (Tauri Desktop)      │
       │         ┌──────────────────┐           │
       └────────►│  ~/.agents/      │◄──────────┘
                 │  skills/local/   │
                 │  (SSOT Hub)      │
                 └────────┬─────────┘
                          │
                 ┌────────┴─────────┐
                 │  skill-manager.db │
                 │  (SQLite)        │
                 └──────────────────┘
```

**Stack:** Tauri v2 (Rust + React 19 + TypeScript + SQLite)

## Installation

### Build from Source

**Prerequisites:**

- [Rust](https://www.rust-lang.org/tools/install) (2021 edition)
- [Node.js](https://nodejs.org/) >= 18
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)

**Build:**

```bash
git clone https://github.com/your-org/sync-skills.git
cd sync-skills
npm install
npm run tauri build
```

Build artifacts are in `src-tauri/target/release/`.

### Development Mode

```bash
npm install
npm run tauri dev
```

## Development

```
sync-skills/
├── src/                    # Frontend (React + TypeScript)
│   ├── App.tsx             # Main component (single-file architecture)
│   ├── App.css             # Styles (CSS variable theme system)
│   ├── types.ts            # TypeScript type definitions
│   └── main.tsx            # Entry point
├── src-tauri/
│   └── src/
│       ├── lib.rs          # IPC command entry (22 commands)
│       ├── db.rs           # SQLite data layer
│       ├── scanner.rs      # Skill directory scanner
│       ├── sync.rs         # File sync engine
│       ├── diff.rs         # LCS unified diff
│       ├── hash.rs         # SHA-256 content hashing
│       ├── discovery.rs    # Tool auto-discovery
│       ├── models.rs       # Data models
│       └── settings.rs     # Settings persistence
├── doc/                    # Documentation & PRD
└── plan/                   # Development plans
```

### Verification

```bash
cd src-tauri && cargo check    # Rust type check
npx tsc --noEmit               # TypeScript type check
```

## Roadmap

| Version | Status | Scope |
|---------|--------|-------|
| v0.1.0 | ✅ Done | Core: tool management, skill scanning, SSOT sync, project management |
| v0.2.0 | ✅ Done | Auto-discovery, sorting/filtering, diff detection, diff view |
| v0.3.0 | ✅ Done | Theme switching, i18n, hash stability fixes |
| v0.4.0 | Planned | Skill editor, remote sync, plugin system |

## License

[AGPL-3.0](LICENSE)
