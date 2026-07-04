<p align="center">
  <a href="README.md">中文</a> | <a href="README.en.md">English</a> | <strong>日本語</strong>
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
  <strong>AI コーディングツール間での Skill 同期マネージャー</strong><br>
  一度編集すれば、すべてのツールに反映。
</p>

<p align="center">
  <a href="#機能">機能</a> •
  <a href="#アーキテクチャ">アーキテクチャ</a> •
  <a href="#インストール">インストール</a> •
  <a href="#開発">開発</a> •
  <a href="#ロードマップ">ロードマップ</a>
</p>

---

## なぜ Skill Manager が必要か

AI コーディングアシスタント（Claude Code、Cursor、Windsurf、Cline など）は、すべて `SKILL.md` ファイルを使って再利用可能な知識やワークフローを定義します。複数のツールを併用している場合、Skill ファイルはあちこちのディレクトリに散在し、手動での同期は手間がかかり、ミスも起こりがちです。

Skill Manager はデスクトップ GUI を提供し、すべての Skill を一箇所で管理し、設定されたすべての AI ツールに自動同期します。

## 機能

- **ツール管理** — AI コーディングツールのパスを登録。13 以上の既知ツールを自動検出
- **Skill スキャン** — ディレクトリを再帰的にスキャンし、`SKILL.md` を含むスキルディレクトリをすべて特定
- **SSOT 同期** — `~/.agents/skills/local/` を中心としたハブ＆スポークモデルで各ツールに配布
- **プロジェクト別管理** — プロジェクトごとに独立した Skill セットを構成
- **差分検出** — LCS unified diff ビューを内蔵し、ファイル単位の変更を正確に表示
- **テーマ切替** — ライト / ダーク / システムに従う
- **多言語対応** — 中文 / English
- **アクティビティログ** — すべての操作の完全な監査記録

## アーキテクチャ

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

**技術スタック：** Tauri v2 (Rust + React 19 + TypeScript + SQLite)

## インストール

### ソースからのビルド

**前提条件：**

- [Rust](https://www.rust-lang.org/tools/install)（2021 edition）
- [Node.js](https://nodejs.org/) >= 18
- [Tauri の前提条件](https://v2.tauri.app/start/prerequisites/)

**ビルド手順：**

```bash
git clone https://github.com/your-org/sync-skills.git
cd sync-skills
npm install
npm run tauri build
```

ビルド成果物は `src-tauri/target/release/` に出力されます。

### 開発モード

```bash
npm install
npm run tauri dev
```

## 開発

```
sync-skills/
├── src/                    # フロントエンド (React + TypeScript)
│   ├── App.tsx             # メインコンポーネント（単一ファイル構成）
│   ├── App.css             # スタイル（CSS 変数テーマシステム）
│   ├── types.ts            # TypeScript 型定義
│   └── main.tsx            # エントリーポイント
├── src-tauri/
│   └── src/
│       ├── lib.rs          # IPC コマンドエントリー (22 コマンド)
│       ├── db.rs           # SQLite データレイヤー
│       ├── scanner.rs      # Skill ディレクトリスキャナー
│       ├── sync.rs         # ファイル同期エンジン
│       ├── diff.rs         # LCS unified diff
│       ├── hash.rs         # SHA-256 コンテンツハッシュ
│       ├── discovery.rs    # ツール自動検出
│       ├── models.rs       # データモデル
│       └── settings.rs     # 設定の永続化
├── doc/                    # ドキュメント & PRD
└── plan/                   # 開発計画
```

### 検証

```bash
cd src-tauri && cargo check    # Rust 型チェック
npx tsc --noEmit               # TypeScript 型チェック
```

## ロードマップ

| バージョン | 状態 | 内容 |
|-----------|------|------|
| v0.1.0 | ✅ 完了 | コア機能：ツール管理、Skill スキャン、SSOT 同期、プロジェクト管理 |
| v0.2.0 | ✅ 完了 | 自動検出、ソート/フィルター、差分検出、diff ビュー |
| v0.3.0 | ✅ 完了 | テーマ切替、多言語対応、ハッシュ安定性修正 |
| v0.4.0 | 計画中 | Skill エディター、リモート同期、プラグインシステム |

## ライセンス

[AGPL-3.0](LICENSE)
