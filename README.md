# PixelLens 🔬

**画面上のどこでも、1クリックで色コードを取得。**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8D8?logo=tauri&logoColor=white)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20(未検証)-blue)](#動作確認状況)

---

## PixelLens とは？

PixelLens は、画面上の任意のピクセルの色をリアルタイムに取得し、そのまま HEX・RGB・HSL などのコードとしてクリップボードにコピーできるカラーピッカーツールです。

**こんな場面で活躍します：**

- 🎨 **デザイナー** — Figma や Photoshop の外にある Web ページや画像の色を素早くスポイト。ツールを切り替えずその場でコードをコピーできます。
- 💻 **フロントエンドエンジニア** — ブラウザで確認している配色を、そのまま CSS / SCSS 変数として貼り付け。
- 🖌️ **イラストレーター / 動画クリエイター** — 参考画像の色をメモする作業が、ショートカット一発に。
- 📐 **UI レビュア** — 実装済み画面の色が仕様通りか、ΔE（知覚的色差）付きで確認。

PixelLens は**タスクトレイに常駐**します。使いたいときだけショートカットで呼び出し、終わったら隠す——邪魔にならずいつでも手元に置けるのが特徴です。

---

## Features

| 機能 | 詳細 |
|---|---|
| 🔍 リアルタイム拡大鏡 | 4x〜20x でカーソル下を拡大表示。グリッド表示あり |
| 📋 多彩なコピー形式 | HEX / RGB / HSL / Float / 0x 記法など 9 種類 |
| ⚡ グローバルショートカット | ワンキーで色コードをコピー（どのアプリが前面でも動作） |
| 👁️ 表示 / 非表示トグル | ショートカットで瞬時に呼び出し・隠す |
| 🎨 日本語伝統色名 | JIS 慣用色名・Web カラー 90 色、CIE76 ΔE 色差付き |
| 🌓 ダーク / ライトテーマ | 好みや環境に合わせて切替 |
| 📌 常時最前面表示 | 他のアプリの上に浮かんで常に視認可能 |
| 🗂️ タスクトレイ常駐 | 閉じてもバックグラウンドで待機、アイコンから即復帰 |

---

## Screenshot

<!-- スクリーンショット -->

---

## Copy Formats

設定画面またはコピーボタンから出力形式を選択できます。

| 形式 | 例 | 主な用途 |
|---|---|---|
| HEX 大文字 | `#4A90E2` | CSS / HTML |
| HEX 小文字 | `#4a90e2` | CSS（小文字派） |
| RGB CSS | `rgb(74, 144, 226)` | CSS |
| RGB 数値 | `74, 144, 226` | Photoshop / Figma |
| HSL CSS | `hsl(213, 70%, 59%)` | CSS |
| Float | `0.290, 0.565, 0.886` | GLSL / Unity / Unreal |
| 0x 記法 | `0x4A90E2` | プログラム全般 |
| HEX (#なし) | `4A90E2` | Photoshop カラーピッカー |
| 色名 | `空色` | デザインドキュメント |

---

## Keyboard Shortcuts

| 操作 | Windows | macOS（予定） |
|---|---|---|
| ウィンドウ 表示 / 非表示 | `Ctrl + Alt + C` | `Cmd + Option + C` |
| 現在の色をクリップボードにコピー | `Ctrl + Shift + C` | `Cmd + Shift + C` |

> ショートカットはどのアプリがアクティブな状態でも動作します（グローバルショートカット）。

---

## 動作確認状況

| 環境 | 状況 | 備考 |
|---|---|---|
| **Windows 10 / 11** | ✅ 動作確認済み | 主要開発・テスト環境 |
| **macOS** | ⚠️ 未検証 | 機材不足のため動作確認できていません。コードは macOS 向けに実装していますが、実機での確認を取れていない状態です。動作報告や不具合 Issue 歓迎です。 |
| **Linux (X11 / Wayland)** | 🚧 未対応 | 将来対応予定（Roadmap v0.5 参照） |

---

## Quick Start

### Download

> **バイナリ配布は現在準備中です。**
> 近いうちに [Releases](https://github.com/suzuki-black/pixel-lens/releases) ページからダウンロードできるようにする予定です。
> 今すぐ試したい場合は、以下の「Build from Source」手順でビルドしてください。

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

起動するとシステムトレイにアイコンが表示され、カラーピッカーウィンドウが前面に浮かびます。

- マウスを動かすと、カーソル下のピクセル色がリアルタイムで更新されます。
- ウィンドウが邪魔なときは **隠すボタン（—）** またはショートカットで非表示にできます。
- 再表示したいときは、ショートカットまたはトレイアイコンをクリックします。
- アプリを完全終了するには、**トレイアイコンを右クリック → 終了** を選択します。

---

## Roadmap

| バージョン | 内容 | 状態 |
|---|---|---|
| v0.1 | コア機能（拡大鏡・色取得・コピー・トレイ常駐） | ✅ 完成 |
| v0.2 | カラー履歴（直近 10 色）/ ピンモード（色を固定） | 📋 予定 |
| v0.3 | パレットエクスポート（CSS / SCSS / JSON）/ コントラスト比チェッカー（WCAG） | 📋 予定 |
| v0.4 | カラーハーモニーホイール | 📋 予定 |
| v0.5 | Linux 正式対応（X11 / Wayland） | 📋 予定 |
| v1.0 | プラグイン API | 💭 検討中 |

---

## Tech Stack

| レイヤ | 技術 |
|---|---|
| UI | HTML5 + CSS3 + Vanilla JavaScript |
| アプリシェル | Tauri v2 (Rust) |
| レンダラー | WebView2 (Windows) / WKWebView (macOS) |
| 色差計算 | CIE76 ΔE（知覚的色差） |
| 画面キャプチャ | Win32 GDI BitBlt (Windows) / CGDisplay (macOS) |
| 色名辞書 | JIS Z 8102 慣用色名 + Web カラー（90 エントリ） |

---

## Contributing

バグ報告・機能提案・プルリクエストを歓迎します。特に **macOS での動作報告** は大変助かります。

---

## License

MIT © 2026 suzuki-black — [LICENSE](./LICENSE) を参照してください。
