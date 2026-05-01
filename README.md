# PixelLens 🔬

**A lightweight, always-on-top color picker for designers and developers.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8D8?logo=tauri&logoColor=white)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20(untested)-blue)](#platform-status)

---

## What is PixelLens?

Pick any pixel color on your screen — HEX, RGB, HSL, and more — and copy it to your clipboard in one keystroke.  
PixelLens lives in your **system tray**, always on standby. Call it up with a shortcut, grab the color, hide it again.

**Built for:**

- 🎨 **Designers** — spot colors from any app without switching tools
- 💻 **Frontend developers** — grab CSS values directly from your browser preview
- 🖌️ **Illustrators** — reference colors from any image or video on screen
- 🔍 **UI reviewers** — verify implemented colors against specs with CIE76 ΔE color difference

---

## Why PixelLens?

| | PixelLens | Electron apps | Browser extensions |
|---|---|---|---|
| Binary size | ~4 MB | ~100 MB+ | N/A |
| Runtime | Tauri (Rust + WebView) | Node.js + Chromium | Browser only |
| System tray resident | ✅ | varies | ❌ |
| Global shortcut | ✅ | varies | ❌ |
| Japanese color names | ✅ | ❌ | ❌ |
| Telemetry | None | varies | varies |
| Open source | ✅ MIT | varies | varies |

---

## Features

| Feature | Detail |
|---|---|
| 🔍 Real-time magnifier | 4x–20x zoom with pixel grid |
| 📋 9 copy formats | HEX / RGB / HSL / Float / 0x and more |
| ⚡ Global shortcuts | Works from any app in the foreground |
| 🌐 EN / JA UI | English and Japanese interface |
| 🎌 Japanese color names | JIS traditional + Web colors (90 entries) |
| 🔤 3-axis color display | Romaji / 漢字 / English (EN mode) |
| 🌓 Dark / Light theme | |
| 📌 Always on top | Floats above all other windows |
| 🗂️ System tray resident | Hides to tray, recalled instantly via shortcut |

---

## Screenshot

<!-- Screenshots coming soon -->

---

## Installation

### Download

> **Prebuilt binaries are not yet available.**  
> We plan to publish installers on the [Releases](https://github.com/suzuki-black/pixel-lens/releases) page soon.  
> In the meantime, build from source using the instructions below.

### Build from Source

**Prerequisites:**

| Platform | Requirement |
|---|---|
| All | [Rust (stable)](https://rustup.rs), [Node.js LTS](https://nodejs.org) |
| Windows | Visual Studio Build Tools 2022 — **"Desktop development with C++"** workload |
| macOS | Xcode Command Line Tools (`xcode-select --install`) |
| Linux | `libgtk-3-dev libwebkit2gtk-4.1-dev` (untested) |

```bash
git clone https://github.com/suzuki-black/pixel-lens
cd pixel-lens
npm install
npm run build
```

Output: `src-tauri/target/release/pixel-lens` (or `.exe` on Windows).

---

## Usage

1. Launch PixelLens — it appears as a floating window and a tray icon.
2. Move your mouse over any pixel to see the color update in real time.
3. Click a copy button or press **Ctrl+Shift+C** to copy the color.
4. Click **—** or press **Ctrl+Alt+C** to hide the window to the tray.
5. To quit, **right-click the tray icon → Quit**.

---

## Keyboard Shortcuts

| Action | Windows | macOS (planned) |
|---|---|---|
| Show / Hide window | `Ctrl + Alt + C` | `Cmd + Option + C` |
| Quick copy current color | `Ctrl + Shift + C` | `Cmd + Shift + C` |

> Shortcuts are global — they work even when PixelLens is hidden or another app is active.

---

## Copy Formats

| Format | Example | Use case |
|---|---|---|
| HEX uppercase | `#4A90E2` | CSS / HTML |
| HEX lowercase | `#4a90e2` | CSS (lowercase preference) |
| RGB CSS | `rgb(74, 144, 226)` | CSS |
| RGB values | `74, 144, 226` | Photoshop / Figma |
| HSL CSS | `hsl(213, 70%, 59%)` | CSS |
| Float | `0.290, 0.565, 0.886` | GLSL / Unity / Unreal |
| 0x notation | `0x4A90E2` | General programming |
| HEX no hash | `4A90E2` | Photoshop color picker |
| Color name | `Sky Blue` / `空色` | Design documentation |

---

## Platform Status

| Platform | Status | Notes |
|---|---|---|
| **Windows 10 / 11** | ✅ Verified | Primary development target |
| **macOS** | ⚠️ Unverified | Implemented but untested due to hardware shortage. Bug reports welcome. |
| **Linux (X11 / Wayland)** | 🚧 Partial | X11 capture fails on WSL2/Xwayland; planned for v0.5 |

---

## Tech Stack

| Layer | Technology |
|---|---|
| UI | HTML5 + CSS3 + Vanilla JavaScript |
| App shell | Tauri v2 (Rust) |
| Renderer | WebView2 (Windows) / WKWebView (macOS) |
| Color difference | CIE76 ΔE |
| Screen capture | Win32 GDI BitBlt (Windows) / CGDisplay (macOS) |
| Color dictionary | JIS Z 8102 + Web colors, 90 entries |

---

## Roadmap

| Version | Content | Status |
|---|---|---|
| v0.1 | Core (magnifier, capture, copy, tray) | ✅ Done |
| v0.2 | EN/JA UI · compact layout · settings persistence · 3-axis color names | ✅ Done |
| v0.3 | Color history (last 10) · pin mode | 📋 Planned |
| v0.4 | Palette export (CSS / SCSS / JSON) · WCAG contrast checker | 📋 Planned |
| v0.5 | Linux support (X11 / Wayland) | 📋 Planned |
| v1.0 | Plugin API | 💭 Under consideration |

---

## Contributing

Bug reports, feature requests, and pull requests are welcome.  
**macOS compatibility reports** are especially appreciated — we currently have no hardware to test on.

---

## License

MIT © 2026 suzuki-black — see [LICENSE](./LICENSE).

---
---

# PixelLens 🔬（日本語）

**デザイナー・開発者のための軽量カラーピッカー。**

---

## PixelLens とは？

画面上の任意のピクセルの色を HEX・RGB・HSL などでワンキーコピー。  
タスクトレイに常駐し、必要なときだけショートカットで呼び出せます。

**こんな方に：**

- 🎨 **デザイナー** — ツールを切り替えずに画面上の色をスポイト
- 💻 **フロントエンドエンジニア** — ブラウザの配色をそのまま CSS 変数に
- 🖌️ **イラストレーター** — 参考画像の色をショートカット一発でメモ
- 🔍 **UI レビュア** — ΔE 色差付きで実装色を仕様と比較

---

## なぜ PixelLens？

- **軽量** — Tauri (Rust) 製、バイナリ約 4 MB
- **テレメトリなし** — 完全オープンソース (MIT)
- **トレイ常駐** — 邪魔にならず、ショートカットですぐ呼び出し
- **9 種のコピー形式** — CSS・Figma・Unity・Photoshop に対応
- **日本語色名** — JIS 慣用色名 + Web カラー 90 色 (CIE76 ΔE)
- **EN / JA 切り替え** — 英語・日本語 UI を設定から変更可能
- **3 軸色名表示 (EN)** — ローマ字 / 漢字 / 英語で色名を表示

---

## インストール

### ダウンロード

> **バイナリ配布は現在準備中です。**  
> 近いうちに [Releases](https://github.com/suzuki-black/pixel-lens/releases) ページで公開予定です。  
> 今すぐ使いたい場合はソースからビルドしてください。

### ソースからビルド

```bash
git clone https://github.com/suzuki-black/pixel-lens
cd pixel-lens
npm install
npm run build
```

**前提条件：**

| 環境 | 必要なもの |
|---|---|
| 全環境 | [Rust (stable)](https://rustup.rs)、[Node.js LTS](https://nodejs.org) |
| Windows | Visual Studio Build Tools 2022（「C++ によるデスクトップ開発」ワークロード） |
| macOS | Xcode Command Line Tools |

---

## 使い方

1. 起動するとウィンドウが表示され、トレイアイコンが現れます。
2. マウスを動かすと色がリアルタイム更新されます。
3. コピーボタンか **Ctrl+Shift+C** で色をコピー。
4. **—** ボタンか **Ctrl+Alt+C** でウィンドウをトレイに隠す。
5. 終了は **トレイアイコン右クリック → PixelLens を終了**。

---

## キーボードショートカット

| 操作 | Windows | macOS（予定） |
|---|---|---|
| ウィンドウ 表示 / 非表示 | `Ctrl + Alt + C` | `Cmd + Option + C` |
| クイックコピー | `Ctrl + Shift + C` | `Cmd + Shift + C` |

---

## 動作確認状況

| 環境 | 状況 | 備考 |
|---|---|---|
| **Windows 10 / 11** | ✅ 確認済み | 主要開発・テスト環境 |
| **macOS** | ⚠️ 未検証 | 機材不足のため未確認。不具合報告歓迎 |
| **Linux (X11 / Wayland)** | 🚧 未対応 | v0.5 で対応予定 |

---

## ライセンス

MIT © 2026 suzuki-black — [LICENSE](./LICENSE) を参照。
