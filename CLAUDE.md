# PixelLens — Claude 作業ルール

## 🚨 破壊的コマンドの禁止ルール

### tccutil reset — 必ず bundle ID を指定すること

```bash
# ✅ OK: PixelLens だけリセット
tccutil reset ScreenCapture dev.pixellens.pixellens

# ❌ 絶対禁止: 全アプリの権限を消す（過去に実行してユーザーに多大な迷惑をかけた）
tccutil reset ScreenCapture
tccutil reset All
```

bundle ID なしの `tccutil reset <service>` は **全アプリの権限を消す破壊的操作** であり、
実行前にユーザーへの確認が必要な操作であっても、この形式は使用しない。

---

### 一般的な破壊的コマンドの原則

- `rm -rf` はパスを必ず明示。ワイルドカードや短いパスは慎重に。
- `git reset --hard` / `git clean -f` は実行前にユーザー確認必須。
- システム全体に影響するコマンド（引数なし or ワイルドカード）は実行前に必ず範囲を確認する。
- 「PixelLens だけに影響する操作」のつもりでも、引数を省略すると全体に波及する系統のコマンドは特に注意。

---

## プロジェクト概要

- **Tauri v2** アプリ（macOS / Windows / Linux ハイブリッド）
- macOS では **ScreenCaptureKit (SCKit)** を使用（`capture_helper.m`）
- Windows では **Win32 GDI** を使用（`capture.rs` 内 Windows ブロック）
- Linux では **x11rb** を使用（`capture.rs` 内 Linux ブロック）
- macOS 固有の修正を行う際は Windows / Linux への影響がないか `#[cfg(target_os = "macos")]` / `#ifdef` で分岐されているか確認する

## ビルド・インストール

```bash
./install.sh   # ビルド + /Applications インストール + entitlements 再署名
```

Tauri の adhoc ビルドは entitlements をバイナリに埋め込まない。
`install.sh` 内の `codesign --force --deep --sign - --entitlements ...` が必須。

**`install.sh` に `tccutil reset` を入れてはいけない。** 毎回リセットされて
ユーザーが再認証を繰り返すことになる。TCC リセットは手動で必要な時だけ実行する。

## macOS 画面収録権限フロー

1. 起動 2 秒後に `sc_show_picker()` が自動呼び出しされ `SCContentSharingPicker` を表示
2. ユーザーが「ディスプレイ全体」を選択
3. `didUpdateWithFilter:forStream:` で全画面 filter を取得し SCStream 開始
4. 以降はピッカーで選択したセッション中は正常にキャプチャできる

### ⚠️ System Settings に PixelLens が表示されないのは正常

SCContentSharingPicker はセッションベースの独自認証機構を使用しており、
従来の TCC（System Settings → 画面収録リスト）とは**別サービス**。
そのため PixelLens は System Settings の一覧に現れないが、これは**バグではなく正常動作**。
実際のキャプチャは正しく動作する（macOS 26 で動作確認済み）。

`CGPreflightScreenCaptureAccess()` は常に false を返す（旧 TCC サービス用のため無関係）。

権限がない場合（ピッカー未実行）`sc_capture_rect_rgba` は NULL を返し、黒画像になる。
