#!/bin/bash
# install.sh — PixelLens ビルド＆インストールスクリプト
#
# 実行方法: ./install.sh
#
# このスクリプトは以下を行います:
#   1. Tauri でリリースビルド
#   2. /Applications にインストール
#   3. entitlements.plist を付けて再署名 (com.apple.security.screen-recording を含む)
#   4. TCC の古い記録をリセット

set -e
cd "$(dirname "$0")"

BUNDLE_PATH="src-tauri/target/release/bundle/macos/PixelLens.app"
ENTITLEMENTS="src-tauri/entitlements.plist"
INSTALL_PATH="/Applications/PixelLens.app"

echo "==> PixelLens ビルド中..."
npx tauri build

echo "==> /Applications にインストール中..."
rm -rf "$INSTALL_PATH"
cp -R "$BUNDLE_PATH" "$INSTALL_PATH"

echo "==> entitlements.plist を付けて再署名中..."
codesign --force --deep --sign - \
  --entitlements "$ENTITLEMENTS" \
  "$INSTALL_PATH"

echo "==> 署名確認:"
codesign --display --xml --entitlements - "$INSTALL_PATH" 2>&1 | grep -A2 "screen-recording" || true

echo ""
echo "✅ インストール完了"
echo "   /Applications/PixelLens.app"
echo ""
echo "  PixelLens を再起動してください（既に起動中の場合はトレイメニューから終了 → 再起動）。"
echo "  初回起動時のみ、起動後 2 秒で「画面収録」ピッカーが自動表示されます。"
echo "  「ディスプレイ全体」を選択して許可してください。"
echo ""
echo "  ※ TCC リセットが必要な場合のみ手動で実行:"
echo "     tccutil reset ScreenCapture dev.pixellens.pixellens"
