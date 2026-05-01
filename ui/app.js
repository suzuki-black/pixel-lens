/* global __TAURI__ */

(function () {

// ── i18n ─────────────────────────────────────────────────────────────────────
const I18N = {
  en: {
    settings:        'Settings',
    hide:            'Hide to tray',
    back:            'Back',
    save:            'Save',
    theme:           'Theme',
    dark:            'Dark',
    light:           'Light',
    zoom:            'Zoom',
    dictionary:      'Color Dictionary',
    jis:             'JIS',
    web:             'Web',
    language:        'Language',
    quickCopyFormat: 'Quick Copy Format',
    shortcut:        'Shortcut',
    shortcutHint:    'Customization coming in next version',
    gridTooltip:     'Toggle grid',
    copySuccess:     'Copied: ',
    copyFail:        'Copy failed',
    settingsSaved:   'Settings saved',
    settingsFailed:  'Failed to save',
    permWarning:     'Screen recording permission required. Go to System Preferences › Privacy & Security › Screen Recording.',
  },
  ja: {
    settings:        '設定',
    hide:            'トレイへ',
    back:            '戻る',
    save:            '保存',
    theme:           'テーマ',
    dark:            'ダーク',
    light:           'ライト',
    zoom:            '拡大倍率',
    dictionary:      '色名辞書',
    jis:             'JIS慣用色',
    web:             'Webカラー',
    language:        '言語',
    quickCopyFormat: 'クイックコピー形式',
    shortcut:        'ショートカット',
    shortcutHint:    'クリックして変更（次バージョン対応予定）',
    gridTooltip:     'グリッド表示',
    copySuccess:     'コピー: ',
    copyFail:        'コピー失敗',
    settingsSaved:   '設定を保存しました',
    settingsFailed:  '保存に失敗しました',
    permWarning:     '画面収録の権限が必要です。システム環境設定 › プライバシーとセキュリティ › 画面収録 で許可してください。',
  },
};

function t(key) {
  const lang = state.settings.language || 'en';
  return (I18N[lang] || I18N.en)[key] || key;
}

function applyI18n() {
  // Text content
  document.querySelectorAll('[data-i18n]').forEach(el => {
    el.textContent = t(el.dataset.i18n);
  });
  // title attributes
  document.querySelectorAll('[data-i18n-title]').forEach(el => {
    el.title = t(el.dataset.i18nTitle);
  });
  // Grid button tooltip
  const gridBtn = document.getElementById('btn-toggle-grid');
  if (gridBtn) gridBtn.title = t('gridTooltip');
  // Permission warning text
  const permText = document.getElementById('permission-warning-text');
  if (permText) permText.textContent = t('permWarning');
  // html lang attribute
  document.documentElement.lang = state.settings.language || 'en';
}

// ── Diagnostics ───────────────────────────────────────────────────────────────
function diag(msg) {
  const text = '[JS] ' + msg;
  console.log(text);
  try {
    const _invoke = window.__TAURI_INTERNALS__?.invoke
      || window.__TAURI__?.core?.invoke;
    if (typeof _invoke === 'function') {
      _invoke('js_log', { level: 'INFO', msg }).catch(() => {});
    }
  } catch (_) {}
}

window.addEventListener('unhandledrejection', (e) => {
  diag('unhandledrejection: ' + String(e.reason));
});

diag('script-start');
document.getElementById('coord-display').textContent = 'app.js-loaded';
diag('coord-display set');

// ── Tauri bridge ──────────────────────────────────────────────────────────────
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

// ── Color format conversion ───────────────────────────────────────────────────
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
  const { r, g, b, hex, nearest_name, nearest_en } = color;
  const hexUp = hex.replace('#', '').toUpperCase();
  const [h, s, l] = rgbToHsl(r, g, b);
  const name = (state.settings.language === 'en') ? nearest_en : nearest_name;
  switch (fmt) {
    case 'hex':         return `#${hexUp}`;
    case 'hex_lower':   return hex.toLowerCase();
    case 'rgb':         return `rgb(${r}, ${g}, ${b})`;
    case 'rgb_raw':     return `${r}, ${g}, ${b}`;
    case 'hsl':         return `hsl(${h}, ${s}%, ${l}%)`;
    case 'float':       return `${(r/255).toFixed(3)}, ${(g/255).toFixed(3)}, ${(b/255).toFixed(3)}`;
    case 'hex_0x':      return `0x${hexUp}`;
    case 'hex_no_hash': return hexUp;
    case 'name':        return name;
    default:            return `#${hexUp}`;
  }
}

// ── Mock for browser preview ──────────────────────────────────────────────────
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
        nearest_romaji: 'Sora-iro',
        nearest_en: 'Sky Blue',
        nearest_hex: '#4A90D9',
        delta_e: 1.2,
      },
      cursor_x: 640,
      cursor_y: 400,
    };
  }
  if (cmd === 'get_settings') {
    return {
      zoom_level: 10, use_jis_colors: true,
      shortcut: 'Ctrl+Alt+C', copy_shortcut: 'Ctrl+Shift+C',
      copy_format: 'hex', theme: 'dark', show_grid: true, language: 'en',
    };
  }
  if (cmd === 'save_settings') return null;
  return null;
}

// ── State ─────────────────────────────────────────────────────────────────────
const state = {
  running: false,
  rafId: null,
  currentColor: {
    r: 0, g: 0, b: 0,
    hex: '#000000',
    nearest_name: '—', nearest_romaji: '—', nearest_en: '—',
    nearest_hex: '#000000', delta_e: 0,
  },
  settings: {
    zoom_level: 10, use_jis_colors: true,
    shortcut: 'Ctrl+Alt+C', copy_shortcut: 'Ctrl+Shift+C',
    copy_format: 'hex', theme: 'dark', show_grid: true, language: 'en',
  },
  permissionError: false,
};

// ── DOM refs ──────────────────────────────────────────────────────────────────
const canvas       = document.getElementById('magnifier-canvas');
const ctx          = canvas.getContext('2d');
const coordDisplay = document.getElementById('coord-display');
const colorSwatch  = document.getElementById('color-swatch');
const colorName    = document.getElementById('color-name');
const colorNameSub = document.getElementById('color-name-sub');
const valCombined  = document.getElementById('val-combined');
const toast        = document.getElementById('toast');
const permWarn     = document.getElementById('permission-warning');
const gridBtn      = document.getElementById('btn-toggle-grid');

// ── Magnifier rendering ───────────────────────────────────────────────────────
const CANVAS_SIZE = 200;
const CAPTURE_PX  = 21;

let tickCount        = 0;
let captureFailCount = 0;
const CAPTURE_FAIL_MAX = 5;

async function tick() {
  if (!state.running) return;

  try {
    if (tickCount < 3) diag('tick#' + tickCount);
    tickCount++;

    const pos = await invoke('get_cursor_pos');
    coordDisplay.textContent = `X: ${pos.x}  Y: ${pos.y}`;

    const data = await invoke('capture_area', { cx: pos.x, cy: pos.y, size: CAPTURE_PX });

    if (data.image_b64) {
      const img = await loadImage(`data:image/png;base64,${data.image_b64}`);
      renderMagnifier(img, CAPTURE_PX);
    } else {
      renderPlaceholder(pos.x, pos.y);
    }

    updateColorDisplay(data.color);
    permWarn.classList.add('hidden');
    state.permissionError = false;
    captureFailCount = 0;

  } catch (err) {
    const msg = String(err);
    console.error('[PixelLens tick error]', msg);
    captureFailCount++;

    if (captureFailCount >= CAPTURE_FAIL_MAX) {
      coordDisplay.textContent = 'Screen capture N/A (WSL2?)';
      setTimeout(tick, 2000);
      return;
    }

    coordDisplay.textContent = `Error: ${msg.substring(0, 60)}`;
    if (msg.includes('permission') || msg.includes('CGDisplay') || msg.includes('access')) {
      if (!state.permissionError) {
        state.permissionError = true;
        permWarn.classList.remove('hidden');
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
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, 0, 0, capturedPx, capturedPx, 0, 0, CANVAS_SIZE, CANVAS_SIZE);
  const zoom = CANVAS_SIZE / capturedPx;
  if (state.settings.show_grid && zoom >= 6) drawGrid(zoom, capturedPx);
}

function drawGrid(cellSize, count) {
  ctx.strokeStyle = 'rgba(0,0,0,0.18)';
  ctx.lineWidth = 0.5;
  for (let i = 1; i < count; i++) {
    const x = i * cellSize;
    ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, CANVAS_SIZE); ctx.stroke();
    const y = i * cellSize;
    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(CANVAS_SIZE, y); ctx.stroke();
  }
}

function renderPlaceholder(cx, cy) {
  ctx.fillStyle = getComputedStyle(document.documentElement)
    .getPropertyValue('--bg-elevated').trim() || '#313139';
  ctx.fillRect(0, 0, CANVAS_SIZE, CANVAS_SIZE);
  ctx.fillStyle = 'rgba(255,255,255,0.1)';
  const cell = 20;
  for (let r = 0; r < CANVAS_SIZE / cell; r++) {
    for (let c = 0; c < CANVAS_SIZE / cell; c++) {
      if ((r + c) % 2 === 0) ctx.fillRect(c * cell, r * cell, cell, cell);
    }
  }
  ctx.fillStyle = 'rgba(255,255,255,0.35)';
  ctx.font = '10px sans-serif';
  ctx.textAlign = 'center';
  ctx.fillText('Magnifier (Tauri only)', CANVAS_SIZE / 2, CANVAS_SIZE / 2);
}

// ── Color display ─────────────────────────────────────────────────────────────
function updateColorDisplay(color) {
  state.currentColor = color;
  colorSwatch.style.backgroundColor = color.hex;

  const lang = state.settings.language || 'en';

  if (lang === 'en') {
    // 3-axis: Romaji / Japanese / English
    colorName.textContent = `${color.nearest_romaji} / ${color.nearest_name} / ${color.nearest_en}`;
    colorNameSub.textContent = color.delta_e > 0.5 ? `ΔE ${color.delta_e.toFixed(1)}` : '';
  } else {
    // JA: Japanese name with inline ΔE
    const de = color.delta_e > 0.5 ? `（ΔE ${color.delta_e.toFixed(1)}）` : '';
    colorName.textContent = `${color.nearest_name}${de}`;
    colorNameSub.textContent = '';
  }

  // Consolidated value row
  const nameLabel = lang === 'en' ? color.nearest_en : color.nearest_name;
  valCombined.textContent = `${color.hex}  |  ${color.r}, ${color.g}, ${color.b}  |  ${nameLabel}`;
}

// ── Quick copy ────────────────────────────────────────────────────────────────
async function quickCopy() {
  const text = formatColor(state.currentColor, state.settings.copy_format);
  try {
    await writeText(text);
    showToast(t('copySuccess') + text);
    const btn = document.getElementById('btn-copy-main');
    if (btn) { btn.classList.add('copied'); setTimeout(() => btn.classList.remove('copied'), 1200); }
  } catch (e) {
    showToast(t('copyFail'));
  }
}

// ── Grid toggle (icon button) ─────────────────────────────────────────────────
gridBtn.addEventListener('click', () => {
  state.settings.show_grid = !state.settings.show_grid;
  gridBtn.setAttribute('aria-pressed', String(state.settings.show_grid));
  gridBtn.classList.toggle('active', state.settings.show_grid);
});

// ── Hide button ───────────────────────────────────────────────────────────────
document.getElementById('btn-hide').addEventListener('click', () => {
  invoke('hide_window').catch(() => {});
});

// ── Copy button ───────────────────────────────────────────────────────────────
document.getElementById('btn-copy-main').addEventListener('click', () => quickCopy());

// ── Global shortcut event (Ctrl+Shift+C) ─────────────────────────────────────
if (isTauri && window.__TAURI__?.event?.listen) {
  window.__TAURI__.event.listen('quick-copy', () => quickCopy());
}

// ── Toast ─────────────────────────────────────────────────────────────────────
let toastTimer = null;
function showToast(msg) {
  toast.textContent = msg;
  toast.classList.add('visible');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('visible'), 2000);
}

// ── Navigation ────────────────────────────────────────────────────────────────
const viewMain     = document.getElementById('view-main');
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
const zoomSlider      = document.getElementById('zoom-slider');
const zoomLabel       = document.getElementById('zoom-label');
const shortcutInput   = document.getElementById('shortcut-input');
const copyFormatSelect= document.getElementById('copy-format-select');

zoomSlider.addEventListener('input', () => {
  zoomLabel.textContent = `${zoomSlider.value}x`;
});

function setupSegmented(containerId, onChange) {
  const container = document.getElementById(containerId);
  if (!container) return;
  container.querySelectorAll('.seg-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      container.querySelectorAll('.seg-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      onChange(btn.dataset.value);
    });
  });
}

let pendingTheme = state.settings.theme;
let pendingDict  = state.settings.use_jis_colors ? 'jis' : 'web';
let pendingLang  = state.settings.language;

setupSegmented('theme-selector', (val) => { pendingTheme = val; });
setupSegmented('dict-selector',  (val) => { pendingDict  = val; });
setupSegmented('lang-selector',  (val) => { pendingLang  = val; });

function loadSettingsUI() {
  const s = state.settings;
  zoomSlider.value = s.zoom_level;
  zoomLabel.textContent = `${s.zoom_level}x`;
  shortcutInput.value = s.shortcut;
  copyFormatSelect.value = s.copy_format || 'hex';

  pendingTheme = s.theme;
  pendingDict  = s.use_jis_colors ? 'jis' : 'web';
  pendingLang  = s.language || 'en';

  setActiveSegBtn('theme-selector', s.theme);
  setActiveSegBtn('dict-selector',  s.use_jis_colors ? 'jis' : 'web');
  setActiveSegBtn('lang-selector',  s.language || 'en');
}

function setActiveSegBtn(containerId, value) {
  const container = document.getElementById(containerId);
  if (!container) return;
  container.querySelectorAll('.seg-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.value === value);
  });
}

document.getElementById('btn-save-settings').addEventListener('click', async () => {
  const newSettings = {
    zoom_level:     parseInt(zoomSlider.value, 10),
    use_jis_colors: pendingDict === 'jis',
    shortcut:       shortcutInput.value,
    theme:          pendingTheme,
    show_grid:      state.settings.show_grid,
    copy_format:    copyFormatSelect.value,
    copy_shortcut:  state.settings.copy_shortcut,
    language:       pendingLang,
  };

  try {
    await invoke('save_settings', { settings: newSettings });
    state.settings = newSettings;
    applyTheme(newSettings.theme);
    applyI18n();
    updateColorDisplay(state.currentColor); // re-render with new language
    showToast(t('settingsSaved'));
    viewSettings.classList.remove('active');
    viewMain.classList.add('active');
  } catch (e) {
    showToast(t('settingsFailed'));
  }
});

function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme);
}

// ── Init ──────────────────────────────────────────────────────────────────────
async function init() {
  diag('init: start');
  try {
    const s = await invoke('get_settings');
    diag('init: get_settings OK theme=' + s.theme + ' lang=' + s.language);
    state.settings = s;
    gridBtn.setAttribute('aria-pressed', String(s.show_grid));
    gridBtn.classList.toggle('active', s.show_grid);
    applyTheme(s.theme);
    applyI18n();
  } catch (e) {
    diag('init: get_settings FAILED: ' + String(e));
    applyI18n(); // apply defaults
  }

  diag('init: starting tick loop');
  state.running = true;
  tick();
}

init();

})(); // end IIFE
