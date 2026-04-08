# PixelLens 🔬

**ピクセル単位の色情報を、すぐそこに。**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8D8?logo=tauri&logoColor=white)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-blue)](https://github.com/suzuki-black/pixel-lens/releases)

---

## Features

- 🔍 **リアルタイムピクセルカラー取得・拡大鏡**（4x〜20x）
- 📋 **HEX / RGB / HSL / Float など多彩なコピー形式**
- 🎨 **日本語伝統色名対応**（JIS慣用色名・Webカラー 90色）CIE76 ΔE 知覚的色差
- ⚡ **グローバルショートカット一発コピー**（Ctrl+Shift+C）
- 👁️ **ウィンドウ表示/非表示トグル**（Ctrl+Alt+C）
- 🌓 **ダーク / ライトテーマ**
- 📌 **常時最前面表示**
- 💻 **クロスプラットフォーム**（Windows・macOS）

---

## Screenshot

<!-- スクリーンショット -->

---

## Quick Start

### Download

最新バイナリは [Releases](https://github.com/suzuki-black/pixel-lens/releases) ページからダウンロードできます。

### Build from Source

**Prerequisites:**

- [Rust (stable)](https://rustup.rs)
- [Node.js (LTS)](https://nodejs.org)
- **Windows:** Visual Studio Build Tools 2022（「C++ によるデスクトップ開発」ワークロード必須）
- **macOS:** Xcode Command Line Tools

```bash
git clone https://github.com/suzuki-black/pixel-lens
cd pixel-lens
npm install
npm run build
```

ビルド成果物は `src-tauri/target/release/` に生成されます。

---

## Usage

アプリを起動すると画面上に常時最前面のカラーピッカーウィンドウが表示されます。
マウスを動かすと、カーソル下のピクセル色がリアルタイムで更新されます。

### Keyboard Shortcuts

| ショートカット | 動作 |
|---|---|
| `Ctrl+Alt+C` | ウィンドウ 表示 / 非表示 |
| `Ctrl+Shift+C` | 現在の色をクリップボードにコピー |

---

## Copy Formats

ウィンドウ内のフォーマットセレクターで出力形式を選択できます。

| 形式 | 例 | 用途 |
|---|---|---|
| HEX (#FFFFFF) | `#4A90E2` | CSS / HTML |
| HEX 小文字 | `#4a90e2` | CSS |
| RGB — CSS | `rgb(74, 144, 226)` | CSS |
| RGB — 数値 | `74, 144, 226` | Photoshop / Figma |
| HSL — CSS | `hsl(213, 70%, 59%)` | CSS |
| Float | `0.290, 0.565, 0.886` | GLSL / Unity / Unreal |
| 0x 記法 | `0x4A90E2` | プログラム全般 |
| HEX (# なし) | `4A90E2` | Photoshop カラーピッカー |
| 色名 | `空色` | デザインドキュメント |

---

## Roadmap

- [ ] v0.2 — カラー履歴（直近10色）
- [ ] v0.2 — ピンモード（クリックで色を固定）
- [ ] v0.3 — パレットエクスポート（CSS / SCSS / JSON）
- [ ] v0.3 — コントラスト比チェッカー（WCAG AA/AAA）
- [ ] v0.4 — カラーハーモニーホイール
- [ ] v0.5 — Linux 正式対応（X11 / Wayland）
- [ ] v1.0 — プラグイン API

---

## Tech Stack

| レイヤ | 技術 |
|---|---|
| UI | HTML5 + CSS3 + Vanilla JavaScript |
| アプリシェル | Tauri v2 (Rust) |
| レンダラー | WebView2 (Windows) / WKWebView (macOS) |
| 色差計算 | CIE76 ΔE（知覚的色差） |
| 画面キャプチャ | Win32 GDI BitBlt (Windows) / CGDisplay (macOS) |
| 色名辞書 | JIS Z 8102 慣用色名 + Webカラー（90エントリ） |

---

## License

MIT © 2026 suzuki-black — [LICENSE](./LICENSE) を参照してください。
