/* global __TAURI__ */

(function () {

// ── 診断ログ（Rust側に送れる場合は送り、常にconsole.logにも出す）────────────
function diag(msg) {
  const text = '[JS] ' + msg;
  console.log(text);
  // Rust側へ転送（IPC利用可能なら）
  try {
    const _invoke = window.__TAURI_INTERNALS__?.invoke
      || window.__TAURI__?.core?.invoke;
    if (typeof _invoke === 'function') {
      _invoke('js_log', { level: 'INFO', msg }).catch(() => {});
    }
  } catch (_) {}
}

// ── 診断：エラー捕捉（コンソールのみ、UIは汚染しない）────────────────────
window.addEventListener('unhandledrejection', (e) => {
  diag('unhandledrejection: ' + String(e.reason));
});

diag('script-start');
document.getElementById('coord-display').textContent = 'app.js-loaded';
diag('coord-display set');

// ── Tauri bridge (graceful fallback for browser preview) ────────────────────
diag('__TAURI__ type=' + typeof window.__TAURI__);
diag('__TAURI_INTERNALS__ type=' + typeof window.__TAURI_INTERNALS__);

const isTauri = typeof window.__TAURI__ !== 'undefined'
  && typeof window.__TAURI__?.core?.invoke === 'function';

diag('isTauri=' + isTauri);

const invoke = isTauri
  ? (cmd, args) => window.__TAURI__.core.invoke(cmd, args)
  : (cmd, args) => mockInvoke(cmd, args);

const writeText = isTauri
  ? (text) => window.__TAURI__['clipboard-manager'].writeText(text)
  : (text) => navigator.clipboard.writeText(text);

// ── Color format conversion ──────────────────────────────────────────────────
function rgbToHsl(r, g, b) {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  let h = 0, s = 0, l = (max + min) / 2;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case r: h = ((g - b) / d + (g < b ? 6 : 0)) / 6; break;
      case g: h = ((b - r) / d + 2) / 6; break;
      case b: h = ((r - g) / d + 4) / 6; break;
    }
  }
  return [Math.round(h * 360), Math.round(s * 100), Math.round(l * 100)];
}

function formatColor(color, fmt) {
  const { r, g, b, hex, nearest_name } = color;
  const hexUp = hex.replace('#', '').toUpperCase();
  const [h, s, l] = rgbToHsl(r, g, b);
  switch (fmt) {
    case 'hex':         return `#${hexUp}`;
    case 'hex_lower':   return hex.toLowerCase();
    case 'rgb':         return `rgb(${r}, ${g}, ${b})`;
    case 'rgb_raw':     return `${r}, ${g}, ${b}`;
    case 'hsl':         return `hsl(${h}, ${s}%, ${l}%)`;
    case 'float':       return `${(r/255).toFixed(3)}, ${(g/255).toFixed(3)}, ${(b/255).toFixed(3)}`;
    case 'hex_0x':      return `0x${hexUp}`;
    case 'hex_no_hash': return hexUp;
    case 'name':        return nearest_name;
    default:            return `#${hexUp}`;
  }
}

/** Mock responses for browser preview / development */
async function mockInvoke(cmd, args) {
  if (cmd === 'get_cursor_pos') return { x: 640, y: 400 };
  if (cmd === 'capture_area') {
    return {
      image_b64: '',
      width: args.size,
      height: args.size,
      color: {
        r: 74, g: 144, b: 226,
        hex: '#4A90E2',
        nearest_name: '空色',
        nearest_hex: '#4A90E2',
        delta_e: 0.0,
      },
      cursor_x: 640,
      cursor_y: 400,
    };
  }
  if (cmd === 'get_settings') {
    return { zoom_level: 10, use_jis_colors: true, shortcut: 'Ctrl+Alt+C', copy_shortcut: 'Ctrl+Shift+C', copy_format: 'hex', theme: 'dark', show_grid: true };
  }
  if (cmd === 'save_settings') return null;
  return null;
}

// ── State ────────────────────────────────────────────────────────────────────
const state = {
  running: false,
  rafId: null,
  currentColor: { r: 0, g: 0, b: 0, hex: '#000000', nearest_name: '—', nearest_hex: '#000000' },
  settings: { zoom_level: 10, use_jis_colors: true, shortcut: 'Ctrl+Alt+C', copy_shortcut: 'Ctrl+Shift+C', copy_format: 'hex', theme: 'dark', show_grid: true },
  permissionError: false,
};

// ── DOM refs ─────────────────────────────────────────────────────────────────
const canvas = document.getElementById('magnifier-canvas');
const ctx = canvas.getContext('2d');
const coordDisplay = document.getElementById('coord-display');
const colorSwatch = document.getElementById('color-swatch');
const colorName = document.getElementById('color-name');
const colorNameNearest = document.getElementById('color-name-nearest');
const valHex = document.getElementById('val-hex');
const valRgb = document.getElementById('val-rgb');
const valName = document.getElementById('val-name');
const toast = document.getElementById('toast');
const permissionWarning = document.getElementById('permission-warning');
const toggleGrid = document.getElementById('toggle-grid');

// ── Magnifier rendering ──────────────────────────────────────────────────────
const CANVAS_SIZE = 200; // px canvas dimension
const CAPTURE_PX = 21;   // pixels to capture (odd number for center alignment)

let lastImageData = null;

let tickCount = 0;
let captureFailCount = 0;
const CAPTURE_FAIL_MAX = 5; // これを超えたら低速ポーリングに切替

async function tick() {
  if (!state.running) return;

  try {
    // 最初の3回だけ詳細ログ
    if (tickCount < 3) {
      diag('tick#' + tickCount + ' invoke=' + (isTauri ? 'tauri' : 'mock'));
    }
    tickCount++;

    const pos = await invoke('get_cursor_pos');
    if (tickCount <= 3) diag('get_cursor_pos OK x=' + pos.x + ' y=' + pos.y);
    coordDisplay.textContent = `X: ${pos.x}  Y: ${pos.y}`;

    const size = CAPTURE_PX;
    const data = await invoke('capture_area', { cx: pos.x, cy: pos.y, size });
    if (tickCount <= 3) diag('capture_area OK hex=' + (data.color?.hex ?? '?'));

    if (data.image_b64) {
      const img = await loadImage(`data:image/png;base64,${data.image_b64}`);
      renderMagnifier(img, size);
    } else {
      // Browser preview: draw placeholder
      renderPlaceholder(pos.x, pos.y);
    }

    updateColorDisplay(data.color);
    permissionWarning.classList.add('hidden');
    state.permissionError = false;
    captureFailCount = 0; // 成功したらリセット
  } catch (err) {
    const msg = String(err);
    console.error('[PixelLens tick error]', msg);

    captureFailCount++;
    if (captureFailCount >= CAPTURE_FAIL_MAX) {
      // 連続失敗: 低速ポーリングに切替（ログスパム防止）
      coordDisplay.textContent = 'Screen capture N/A (WSL2?)';
      setTimeout(tick, 2000);
      return;
    }

    coordDisplay.textContent = `Error: ${msg.substring(0, 80)}`;
    if (msg.includes('permission') || msg.includes('CGDisplay') || msg.includes('access')) {
      if (!state.permissionError) {
        state.permissionError = true;
        permissionWarning.classList.remove('hidden');
      }
    }
  }

  state.rafId = requestAnimationFrame(tick);
}

function loadImage(src) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

function renderMagnifier(img, capturedPx) {
  ctx.clearRect(0, 0, CANVAS_SIZE, CANVAS_SIZE);

  const zoom = CANVAS_SIZE / capturedPx;
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, 0, 0, capturedPx, capturedPx, 0, 0, CANVAS_SIZE, CANVAS_SIZE);

  if (state.settings.show_grid && zoom >= 6) {
    drawGrid(zoom, capturedPx);
  }
}

function drawGrid(cellSize, count) {
  ctx.strokeStyle = 'rgba(0,0,0,0.18)';
  ctx.lineWidth = 0.5;
  for (let i = 1; i < count; i++) {
    const x = i * cellSize;
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, CANVAS_SIZE);
    ctx.stroke();
    const y = i * cellSize;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(CANVAS_SIZE, y);
    ctx.stroke();
  }
}

function renderPlaceholder(cx, cy) {
  // Browser preview: checkerboard + label
  ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--bg-elevated').trim() || '#313139';
  ctx.fillRect(0, 0, CANVAS_SIZE, CANVAS_SIZE);
  ctx.fillStyle = 'rgba(255,255,255,0.1)';
  const cell = 20;
  for (let r = 0; r < CANVAS_SIZE / cell; r++) {
    for (let c = 0; c < CANVAS_SIZE / cell; c++) {
      if ((r + c) % 2 === 0) ctx.fillRect(c * cell, r * cell, cell, cell);
    }
  }
  ctx.fillStyle = 'rgba(255,255,255,0.4)';
  ctx.font = '11px sans-serif';
  ctx.textAlign = 'center';
  ctx.fillText('拡大鏡 (Tauriで動作)', CANVAS_SIZE / 2, CANVAS_SIZE / 2);
}

function updateColorDisplay(color) {
  state.currentColor = color;
  colorSwatch.style.backgroundColor = color.hex;
  colorName.textContent = color.nearest_name;
  colorNameNearest.textContent = color.delta_e > 0.5
    ? `近似 ΔE ${color.delta_e.toFixed(1)}`
    : '';
  valHex.textContent = color.hex;
  valRgb.textContent = `${color.r}, ${color.g}, ${color.b}`;
  valName.textContent = color.nearest_name;
}

// ── Quick copy (shortcut / global event) ─────────────────────────────────────
async function quickCopy() {
  const text = formatColor(state.currentColor, state.settings.copy_format);
  try {
    await writeText(text);
    showToast(`コピー: ${text}`);
  } catch (e) {
    showToast('コピー失敗');
  }
}

// ── Hide button (タイトルバーなし時のウィンドウ隠しボタン) ──────────────────
document.getElementById('btn-hide').addEventListener('click', () => {
  invoke('hide_window').catch(() => {});
});

// Tauri global shortcut イベント (Ctrl+Shift+C) をリッスン
if (isTauri && window.__TAURI__?.event?.listen) {
  window.__TAURI__.event.listen('quick-copy', () => quickCopy());
}

// ── Copy ─────────────────────────────────────────────────────────────────────
document.querySelectorAll('.copy-btn').forEach(btn => {
  btn.addEventListener('click', async () => {
    const type = btn.dataset.copy;
    let text = '';
    if (type === 'hex') text = state.currentColor.hex;
    else if (type === 'rgb') text = `rgb(${state.currentColor.r}, ${state.currentColor.g}, ${state.currentColor.b})`;
    else if (type === 'name') text = state.currentColor.nearest_name;

    try {
      await writeText(text);
      btn.classList.add('copied');
      setTimeout(() => btn.classList.remove('copied'), 1200);
      showToast(`「${text}」をコピーしました`);
    } catch (e) {
      showToast('コピーに失敗しました');
    }
  });
});

// ── Toast ─────────────────────────────────────────────────────────────────────
let toastTimer = null;

function showToast(msg) {
  toast.textContent = msg;
  toast.classList.add('visible');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('visible'), 2000);
}

// ── Navigation ────────────────────────────────────────────────────────────────
const viewMain = document.getElementById('view-main');
const viewSettings = document.getElementById('view-settings');

document.getElementById('btn-settings').addEventListener('click', () => {
  viewMain.classList.remove('active');
  viewSettings.classList.add('active');
  loadSettingsUI();
});

document.getElementById('btn-back').addEventListener('click', () => {
  viewSettings.classList.remove('active');
  viewMain.classList.add('active');
});

// ── Settings UI ───────────────────────────────────────────────────────────────
const zoomSlider = document.getElementById('zoom-slider');
const zoomLabel = document.getElementById('zoom-label');
const shortcutInput = document.getElementById('shortcut-input');
const copyFormatSelect = document.getElementById('copy-format-select');

zoomSlider.addEventListener('input', () => {
  zoomLabel.textContent = `${zoomSlider.value}x`;
});

function setupSegmented(containerId, onChange) {
  const container = document.getElementById(containerId);
  container.querySelectorAll('.seg-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      container.querySelectorAll('.seg-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      onChange(btn.dataset.value);
    });
  });
}

let pendingTheme = state.settings.theme;
let pendingDict = state.settings.use_jis_colors ? 'jis' : 'web';

setupSegmented('theme-selector', (val) => { pendingTheme = val; });
setupSegmented('dict-selector', (val) => { pendingDict = val; });

function loadSettingsUI() {
  const s = state.settings;
  zoomSlider.value = s.zoom_level;
  zoomLabel.textContent = `${s.zoom_level}x`;
  shortcutInput.value = s.shortcut;
  copyFormatSelect.value = s.copy_format || 'hex';

  pendingTheme = s.theme;
  pendingDict = s.use_jis_colors ? 'jis' : 'web';

  setActiveSegBtn('theme-selector', s.theme);
  setActiveSegBtn('dict-selector', s.use_jis_colors ? 'jis' : 'web');
}

function setActiveSegBtn(containerId, value) {
  const container = document.getElementById(containerId);
  container.querySelectorAll('.seg-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.value === value);
  });
}

document.getElementById('btn-save-settings').addEventListener('click', async () => {
  const newSettings = {
    zoom_level: parseInt(zoomSlider.value, 10),
    use_jis_colors: pendingDict === 'jis',
    shortcut: shortcutInput.value,
    theme: pendingTheme,
    show_grid: state.settings.show_grid,
  };

  newSettings.copy_format = copyFormatSelect.value;
  newSettings.copy_shortcut = state.settings.copy_shortcut;

  try {
    await invoke('save_settings', { settings: newSettings });
    state.settings = newSettings;
    applyTheme(newSettings.theme);
    showToast('設定を保存しました');
    viewSettings.classList.remove('active');
    viewMain.classList.add('active');
  } catch (e) {
    showToast('保存に失敗しました');
  }
});

function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme);
}

// ── Grid toggle ────────────────────────────────────────────────────────────────
toggleGrid.addEventListener('change', () => {
  state.settings.show_grid = toggleGrid.checked;
});

// ── Init ──────────────────────────────────────────────────────────────────────
async function init() {
  diag('init: start');
  try {
    const s = await invoke('get_settings');
    diag('init: get_settings OK theme=' + s.theme);
    state.settings = s;
    toggleGrid.checked = s.show_grid;
    applyTheme(s.theme);
  } catch (e) {
    diag('init: get_settings FAILED: ' + String(e));
    // Use defaults
  }

  diag('init: starting tick loop');
  state.running = true;
  tick();
}

init();

})(); // end IIFE
