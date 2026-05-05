// capture_helper.m — ScreenCaptureKit based screen capture (macOS 26+)
//
// Authorization flow (macOS 26):
//   1. SCContentSharingPicker は TCC 認可を得る唯一の方法。
//      sc_show_picker() でピッカーを提示 → ユーザーがディスプレイを選択
//      → didUpdateWithFilter:forStream: で SCContentFilter を受け取る。
//   2. 受け取った filter で SCStream を開始する。
//      ※ SCScreenshotManager captureImageWithFilter: はピッカー認可では -3801 を返す。
//        picker の filter は SCStream のみで有効。(macOS 15/26 の設計)
//   3. SCStream が継続的にフレームを供給する。最新フレームから
//      カーソル周辺をクロップして返す。
//
// Coordinate mapping:
//   CGEvent.location() → top-left origin, y increases DOWNWARD (Quartz convention).
//   CVPixelBuffer (SCStream output) → top-left origin, row 0 = top of display.
//   Therefore: buf_x = x * scale,  buf_y = y * scale  (no Y-flip needed).
//
// Image orientation:
//   Pixels are copied row-by-row with BGRA→RGBA channel swap.
//   CGContextDrawImage is NOT used (it would flip the image vertically due to
//   CoreGraphics' bottom-left coordinate origin).
//
// Compiled by build.rs on macOS using the `cc` crate.

#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <CoreFoundation/CoreFoundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <stdlib.h>
#import <stdio.h>
#import <stdarg.h>

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------
static void pl_log(const char* fmt, ...) __attribute__((format(printf, 1, 2)));
static void pl_log(const char* fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    fprintf(stderr, "[PixelLens ObjC] %s\n", buf);
    FILE* f = fopen("/private/tmp/pixellens_debug.log", "a");
    if (f) { fprintf(f, "[PixelLens ObjC] %s\n", buf); fclose(f); }
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

/// Active SCStream, started after picker authorization.
/// nil = not yet authorized.
static SCStream*          g_stream          = nil;
static BOOL               g_stream_started  = NO;

/// Latest sample buffer from the stream output delegate.
static CMSampleBufferRef  g_latest_sample   = NULL;
static NSLock*            g_sample_lock     = nil;
static dispatch_queue_t   g_sample_queue    = nil;

/// Display scale factor (backingScaleFactor) stored at stream-start time.
/// Used to map logical cursor coordinates to physical buffer pixels.
/// Direct mapping: buf_x = x * scale,  buf_y = y * scale  (no Y-flip).
static double g_display_scale = 1.0;

/// Suppresses repeated "not authorized" log messages.
static BOOL g_no_auth_logged = NO;

// ---------------------------------------------------------------------------
// SCStreamOutput — accumulates the latest frame
// ---------------------------------------------------------------------------
@interface PLStreamOutput : NSObject <SCStreamOutput>
@end

@implementation PLStreamOutput

- (void)stream:(SCStream*)stream
  didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
  ofType:(SCStreamOutputType)type
{
    if (type != SCStreamOutputTypeScreen) return;
    CMSampleBufferRef retained = (CMSampleBufferRef)CFRetain(sampleBuffer);
    [g_sample_lock lock];
    if (g_latest_sample) CFRelease(g_latest_sample);
    g_latest_sample = retained;
    [g_sample_lock unlock];
}

@end

// ---------------------------------------------------------------------------
// SCStreamDelegate — handles stream errors / unexpected stops
// ---------------------------------------------------------------------------
@interface PLStreamDelegate : NSObject <SCStreamDelegate>
@end

@implementation PLStreamDelegate

- (void)stream:(SCStream*)stream didStopWithError:(NSError*)error {
    pl_log("Stream: stopped with error code=%ld: %s",
           (long)error.code, error.localizedDescription.UTF8String);
    if (g_stream == stream) {
        g_stream         = nil;
        g_stream_started = NO;
        g_no_auth_logged = NO;
        [g_sample_lock lock];
        if (g_latest_sample) { CFRelease(g_latest_sample); g_latest_sample = NULL; }
        [g_sample_lock unlock];
    }
}

@end

static PLStreamOutput*   g_stream_output   = nil;
static PLStreamDelegate* g_stream_delegate = nil;

// ---------------------------------------------------------------------------
// SCContentSharingPicker delegate
// ---------------------------------------------------------------------------
@interface PLPickerDelegate : NSObject <SCContentSharingPickerObserver>
@end

@implementation PLPickerDelegate

// ---------------------------------------------------------------------------
// startStreamWithFilter:scale:physW:physH:
//   Actually create, configure, and start the SCStream.
//   Called either with a picker filter (fallback) or a full-display filter.
// ---------------------------------------------------------------------------
- (void)startStreamWithFilter:(SCContentFilter*)filter
                        scale:(double)scale
                        physW:(size_t)physW
                        physH:(size_t)physH
{
    // Lazy-init singletons.
    if (!g_sample_lock)     g_sample_lock     = [[NSLock alloc] init];
    if (!g_sample_queue)    g_sample_queue    = dispatch_queue_create("dev.pixellens.stream", nil);
    if (!g_stream_output)   g_stream_output   = [[PLStreamOutput alloc] init];
    if (!g_stream_delegate) g_stream_delegate = [[PLStreamDelegate alloc] init];

    g_display_scale  = scale;
    g_no_auth_logged = NO;

    // Configure stream for full-display capture at native resolution.
    SCStreamConfiguration* config = [[SCStreamConfiguration alloc] init];
    config.width                = physW;
    config.height               = physH;
    config.pixelFormat          = kCVPixelFormatType_32BGRA;
    config.showsCursor          = NO;
    config.minimumFrameInterval = CMTimeMake(1, 30); // 30 fps

    SCStream* newStream = [[SCStream alloc] initWithFilter:filter
                                             configuration:config
                                                  delegate:g_stream_delegate];

    NSError* addErr = nil;
    BOOL added = [newStream addStreamOutput:g_stream_output
                                       type:SCStreamOutputTypeScreen
                         sampleHandlerQueue:g_sample_queue
                                      error:&addErr];
    if (!added || addErr) {
        pl_log("Stream: addStreamOutput error: %s",
               addErr ? addErr.localizedDescription.UTF8String : "unknown");
        return;
    }

    g_stream = newStream;

    [newStream startCaptureWithCompletionHandler:^(NSError* err) {
        if (err) {
            pl_log("Stream: startCapture error code=%ld: %s",
                   (long)err.code, err.localizedDescription.UTF8String);
            if (g_stream == newStream) {
                g_stream         = nil;
                g_stream_started = NO;
                g_no_auth_logged = NO;
            }
        } else {
            g_stream_started = YES;
            pl_log("Stream: started OK buf=%zux%zu scale=%.1f",
                   physW, physH, scale);
        }
    }];
}

- (void)contentSharingPicker:(SCContentSharingPicker*)picker
        didUpdateWithFilter:(SCContentFilter*)filter
                 forStream:(SCStream* _Nullable)stream
{
    // The picker's filter might be scoped to a specific window (e.g. the
    // PixelLens window itself) rather than the full display — in that case
    // the SCStream buffer would be all-black outside that window.
    //
    // Strategy: use the picker callback only to confirm authorization, then
    // immediately obtain the full primary display via SCShareableContent and
    // start the stream with that display filter instead.
    // ── Diagnostic: log what kind of filter the picker returned ──────────────
    // contentRect tells us whether this is a display filter or a window filter.
    // Display filter: rect matches full display (e.g. 0,0,1512,982)
    // Window filter:  rect matches a specific window (e.g. 606,248,300,390)
    CGRect fr = filter.contentRect;
    pl_log("Picker: didUpdateWithFilter contentRect=(%.0f,%.0f,%.0f,%.0f) pointPixelScale=%.2f",
           fr.origin.x, fr.origin.y, fr.size.width, fr.size.height,
           (double)filter.pointPixelScale);
    pl_log("Picker: didUpdateWithFilter — obtaining full-display filter via SCShareableContent");

    // Stop the old stream (if any).
    if (g_stream) {
        SCStream* old = g_stream;
        g_stream         = nil;
        g_stream_started = NO;
        [old stopCaptureWithCompletionHandler:^(NSError* e) {
            if (e) pl_log("Stream: stop-old error: %s", e.localizedDescription.UTF8String);
        }];
        [g_sample_lock lock];
        if (g_latest_sample) { CFRelease(g_latest_sample); g_latest_sample = NULL; }
        [g_sample_lock unlock];
    }

    // Capture display geometry for coordinate mapping (done on main thread).
    NSScreen* ms    = [NSScreen mainScreen];
    double scale    = ms ? (double)ms.backingScaleFactor : 2.0;
    double logW     = ms ? ms.frame.size.width  : 1920.0;
    double logH     = ms ? ms.frame.size.height : 1080.0;
    size_t physW    = (size_t)(logW * scale + 0.5);
    size_t physH    = (size_t)(logH * scale + 0.5);
    if (physW < 1) physW = 1;
    if (physH < 1) physH = 1;

    pl_log("Stream: NSScreen=%.0fx%.0f bsf=%.1f → buf=%zux%zu",
           logW, logH, scale, physW, physH);

    // Try to get a full-display SCContentFilter via SCShareableContent.
    // This guarantees the stream captures the entire primary display, regardless
    // of what the picker's filter actually represents.
    CGDirectDisplayID mainID = CGMainDisplayID();
    SCContentFilter* pickerFilter = filter; // keep reference for fallback

    [SCShareableContent getShareableContentWithCompletionHandler:
        ^(SCShareableContent* content, NSError* scErr)
    {
        if (scErr || !content || content.displays.count == 0) {
            pl_log("SCShareableContent: error or empty (%s) — falling back to picker filter",
                   scErr ? scErr.localizedDescription.UTF8String : "no content");
            // Fallback: use the picker's filter directly.
            [self startStreamWithFilter:pickerFilter scale:scale physW:physW physH:physH];
            return;
        }

        // Find the primary display (same CGDirectDisplayID as CGMainDisplayID()).
        SCDisplay* targetDisplay = nil;
        for (SCDisplay* d in content.displays) {
            pl_log("SCShareableContent: display id=%u size=%gx%g",
                   (unsigned)d.displayID,
                   d.frame.size.width, d.frame.size.height);
            if (d.displayID == mainID) {
                targetDisplay = d;
            }
        }
        if (!targetDisplay) {
            targetDisplay = content.displays.firstObject;
            pl_log("SCShareableContent: main display not found, using first: id=%u",
                   (unsigned)targetDisplay.displayID);
        } else {
            pl_log("SCShareableContent: using main display id=%u", (unsigned)mainID);
        }

        // Build a full-display filter (capture everything on that display).
        SCContentFilter* displayFilter =
            [[SCContentFilter alloc] initWithDisplay:targetDisplay
                                   excludingWindows:@[]];

        pl_log("SCShareableContent: created full-display filter — starting stream");
        [self startStreamWithFilter:displayFilter scale:scale physW:physW physH:physH];
    }];
}

- (void)contentSharingPicker:(SCContentSharingPicker*)picker
       didCancelForStream:(SCStream* _Nullable)stream
{
    pl_log("Picker: user cancelled — stream is %s",
           g_stream ? "active" : "nil");
}

- (void)contentSharingPickerStartDidFailWithError:(NSError*)error {
    pl_log("Picker: start failed: %s", error.localizedDescription.UTF8String);
}

@end

static PLPickerDelegate* g_picker_delegate = nil;

// ---------------------------------------------------------------------------
// sc_show_picker — present SCContentSharingPicker (display-only mode)
// ---------------------------------------------------------------------------
void sc_show_picker(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!g_picker_delegate) {
            g_picker_delegate = [[PLPickerDelegate alloc] init];
        }

        SCContentSharingPicker* picker = [SCContentSharingPicker sharedPicker];
        [picker addObserver:g_picker_delegate];

        // ディスプレイ選択のみ許可する (ウィンドウ選択では全画面 filter が得られない)
        SCContentSharingPickerConfiguration* cfg =
            [[SCContentSharingPickerConfiguration alloc] init];
        cfg.allowedPickerModes = SCContentSharingPickerModeSingleDisplay;

        #pragma clang diagnostic push
        #pragma clang diagnostic ignored "-Wnonnull"
        [picker setConfiguration:cfg forStream:nil];
        #pragma clang diagnostic pop

        picker.active = YES;
        [picker present];
        pl_log("Picker: displayed (display-only mode)");
    });
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Capture a rectangle of the screen and return it as an RGBA pixel buffer.
///
/// Parameters (all in CG logical coordinates, top-left origin, y increases DOWN):
///   x, y  — top-left corner of the capture rect in logical pixels
///   w, h  — width and height in logical pixels
///
/// Returns a malloc'd RGBA buffer of (*out_pixel_width × *out_pixel_height × 4) bytes,
/// or NULL if the stream is not running or the buffer is unavailable.
uint8_t* sc_capture_rect_rgba(
    double x, double y, double w, double h,
    uint32_t exclude_win_id,
    uint32_t* out_pixel_width,
    uint32_t* out_pixel_height)
{
    (void)exclude_win_id;

    // SCStream (picker-auth path)
    if (!g_stream || !g_stream_started) {
        if (!g_no_auth_logged) {
            pl_log("SCKit: stream not running — use sc_show_picker() to authorize");
            g_no_auth_logged = YES;
        }
        return NULL;
    }

    // Grab the latest frame under the lock.
    [g_sample_lock lock];
    CMSampleBufferRef sample = g_latest_sample
        ? (CMSampleBufferRef)CFRetain(g_latest_sample)
        : NULL;
    [g_sample_lock unlock];

    if (!sample) {
        // Stream started but no frame delivered yet — transient, just skip.
        return NULL;
    }

    CVImageBufferRef imgBuf = CMSampleBufferGetImageBuffer(sample);
    if (!imgBuf) { CFRelease(sample); return NULL; }

    CVPixelBufferLockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);

    size_t   bufW        = CVPixelBufferGetWidth(imgBuf);
    size_t   bufH        = CVPixelBufferGetHeight(imgBuf);
    size_t   bytesPerRow = CVPixelBufferGetBytesPerRow(imgBuf);
    uint8_t* baseAddr    = (uint8_t*)CVPixelBufferGetBaseAddress(imgBuf);

    // --- Coordinate mapping: logical cursor coords → buffer pixels ---
    //
    // CGEvent.location() returns coordinates with origin at the TOP-LEFT of
    // the main display, y increasing DOWNWARD — identical to the CVPixelBuffer
    // layout (row 0 = top of display).  Direct mapping with no Y-flip:
    //
    //   buf_x = x * scale
    //   buf_y = y * scale
    //
    double scale = (g_display_scale > 0) ? g_display_scale : 1.0;

    double buf_x_f = x * scale;
    double buf_y_f = y * scale;
    double buf_w_f = w * scale;
    double buf_h_f = h * scale;

    // Convert to integers, clamped to buffer bounds.
    size_t bx = (buf_x_f >= 0.0) ? (size_t)(buf_x_f + 0.5) : 0;
    size_t by = (buf_y_f >= 0.0) ? (size_t)(buf_y_f + 0.5) : 0;
    size_t bw = (size_t)(buf_w_f + 0.5);
    size_t bh = (size_t)(buf_h_f + 0.5);
    if (bw < 1) bw = 1;
    if (bh < 1) bh = 1;
    if (bx >= bufW) bx = (bufW > bw) ? bufW - bw : 0;
    if (by >= bufH) by = (bufH > bh) ? bufH - bh : 0;
    if (bx + bw > bufW) bw = bufW - bx;
    if (by + bh > bufH) bh = bufH - by;

    pl_log("SCKit: buf=%zux%zu scale=%.1f "
           "logical=(%.0f,%.0f,%.0f,%.0f) → crop=(%zu,%zu,%zu,%zu)",
           bufW, bufH, scale, x, y, w, h, bx, by, bw, bh);

    if (bw == 0 || bh == 0) {
        pl_log("SCKit: crop region empty after clamping");
        CVPixelBufferUnlockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);
        CFRelease(sample);
        return NULL;
    }

    // Allocate output buffer: RGBA, 4 bytes per pixel.
    size_t   outStride = bw * 4;
    uint8_t* outBuf    = (uint8_t*)malloc(outStride * bh);
    if (!outBuf) {
        CVPixelBufferUnlockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);
        CFRelease(sample);
        return NULL;
    }

    // Copy rows top-to-bottom with BGRA → RGBA channel swap.
    //
    // kCVPixelFormatType_32BGRA byte layout (little-endian):
    //   byte[0] = B,  byte[1] = G,  byte[2] = R,  byte[3] = A
    //
    // Target RGBA:
    //   byte[0] = R,  byte[1] = G,  byte[2] = B,  byte[3] = A (= 255, opaque)
    //
    // NOTE: CGContextDrawImage is intentionally NOT used here.
    //   A standard CGBitmapContext has its coordinate origin at the LOWER-LEFT,
    //   so CGContextDrawImage would place the first image row at the BOTTOM of
    //   the context buffer — i.e., the image would be stored upside down.
    //   Direct memcpy + channel swap preserves the natural top-to-bottom order.
    for (size_t row = 0; row < bh; row++) {
        const uint8_t* src = baseAddr + (by + row) * bytesPerRow + bx * 4;
        uint8_t*       dst = outBuf   +       row  * outStride;
        for (size_t col = 0; col < bw; col++) {
            dst[col * 4 + 0] = src[col * 4 + 2]; // R ← BGRA[2]
            dst[col * 4 + 1] = src[col * 4 + 1]; // G ← BGRA[1]
            dst[col * 4 + 2] = src[col * 4 + 0]; // B ← BGRA[0]
            dst[col * 4 + 3] = 255;               // A  (ignore premultiplied)
        }
    }

    CVPixelBufferUnlockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);
    CFRelease(sample);

    *out_pixel_width  = (uint32_t)bw;
    *out_pixel_height = (uint32_t)bh;
    return outBuf;  // caller (sc_free_buffer) will free() this
}

void sc_free_buffer(uint8_t* buf) {
    free(buf);
}

// Forward declarations (defined later in this file)
void sc_show_tray_menu(void);
void sc_show_context_menu(void); // alias for sc_show_tray_menu

// ---------------------------------------------------------------------------
// PLTrayHandler — native NSStatusItem (bypasses Tauri tray API)
//
// On macOS, Tauri's TrayIconBuilder has a known limitation:
//   • If a menu is attached (statusItem.menu), macOS intercepts ALL clicks
//     and shows the menu — Tauri's on_tray_icon_event never fires.
//   • If no menu is attached, Tauri doesn't configure the button action,
//     so clicks are silently ignored.
// Solution: manage the NSStatusItem entirely in ObjC, with a function
// pointer callback for left-click (toggle window, implemented in Rust).
// ---------------------------------------------------------------------------
static void (*g_tray_left_click_cb)(void) = NULL;

@interface PLTrayHandler : NSObject
+ (instancetype)shared;
- (void)setupWithCallback:(void (*)(void))cb;
- (void)buttonClicked:(id)sender;
@end

@implementation PLTrayHandler {
    NSStatusItem* _item;
}

+ (instancetype)shared {
    static PLTrayHandler* s = nil;
    static dispatch_once_t t;
    dispatch_once(&t, ^{ s = [[PLTrayHandler alloc] init]; });
    return s;
}

- (void)setupWithCallback:(void (*)(void))cb {
    g_tray_left_click_cb = cb;

    _item = [[NSStatusBar systemStatusBar] statusItemWithLength:NSSquareStatusItemLength];

    // Load 32x32.png from the app bundle's Resources/icons/ directory.
    NSImage* icon = nil;
    NSString* iconPath = [[NSBundle mainBundle]
        pathForResource:@"32x32" ofType:@"png" inDirectory:@"icons"];
    if (iconPath) {
        icon = [[NSImage alloc] initWithContentsOfFile:iconPath];
        [icon setSize:NSMakeSize(18, 18)];
        [icon setTemplate:YES]; // auto dark/light appearance
    }
    if (!icon) {
        // Fallback: SF Symbol (macOS 11+)
        icon = [NSImage imageWithSystemSymbolName:@"eyedropper"
                          accessibilityDescription:@"PixelLens"];
    }
    // Force Accessory activation policy so the app never appears in the Dock
    // or Cmd+Tab switcher. LSUIElement in Info.plist is not enough because
    // Tauri calls setActivationPolicy:NSApplicationActivationPolicyRegular
    // during app startup, overriding the plist value.
    [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
    pl_log("Tray: setActivationPolicy → Accessory (no Dock icon)");

    _item.button.image   = icon;
    _item.button.toolTip = @"PixelLens — クリックでメニュー";

    // Left click via standard button action.
    // NOTE: sendActionOn:NSEventMaskRightMouseUp does NOT reliably fire for
    // NSStatusBarButton on macOS — right-click is handled via a local event
    // monitor below instead.
    _item.button.action = @selector(buttonClicked:);
    _item.button.target = self;
    [_item.button sendActionOn:NSEventMaskLeftMouseUp];

    // Right click via local NSEvent monitor.
    // Checks that the event hit our status item's panel window.
    __weak NSStatusItem* weakItem = _item;
    [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskRightMouseDown
                                          handler:^NSEvent*(NSEvent* event) {
        NSStatusItem* item = weakItem;
        if (item && event.window == item.button.window) {
            pl_log("Tray: right click (monitor) → unified menu");
            sc_show_tray_menu();
            return nil; // consume — prevent default AppKit right-click handling
        }
        return event;
    }];

    pl_log("Tray: native NSStatusItem created");
}

- (void)buttonClicked:(id)sender {
    // Both left and right click show the unified menu.
    pl_log("Tray: click → unified menu");
    sc_show_tray_menu();
}

@end

// ---------------------------------------------------------------------------
// sc_request_screen_capture_access
//   1. Call CGRequestScreenCaptureAccess() to register the app in System
//      Settings → Screen Recording (creates the TCC entry even on denial).
//   2. After a short delay, auto-show SCContentSharingPicker so the user can
//      authorize the full-display SCStream without having to click the menu.
//
// Called on a background thread from sc_setup_native_tray.
// ---------------------------------------------------------------------------
static void sc_request_screen_capture_access(void) {
    // CGRequestScreenCaptureAccess registers the app with the TCC daemon
    // so it appears in System Settings → Screen Recording.
    // On macOS 15+ this may show a system dialog; on macOS 26 it shows
    // a prompt that points the user to System Settings.
    // We call it unconditionally so the bundle-ID → TCC row is created.
    bool granted = CGRequestScreenCaptureAccess();
    pl_log("CGRequestScreenCaptureAccess: %s", granted ? "granted" : "not granted (will use SCKit picker)");

    // After 2 seconds, auto-show the SCContentSharingPicker if the stream
    // is not running yet. This covers the common case where the user launches
    // PixelLens for the first time and the picker appears automatically.
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(2.0 * NSEC_PER_SEC)),
                   dispatch_get_main_queue(), ^{
        if (!g_stream_started) {
            pl_log("Auto-showing SCContentSharingPicker on startup");
            sc_show_picker();
        } else {
            pl_log("Stream already running — skipping auto picker");
        }
    });
}

// Called once from Rust setup (dispatches to main queue).
void sc_setup_native_tray(void (*left_click_cb)(void)) {
    dispatch_async(dispatch_get_main_queue(), ^{
        [[PLTrayHandler shared] setupWithCallback:left_click_cb];

        // Request screen capture access in the background so we don't block
        // the main thread. The picker will appear ~2 s after launch.
        dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^{
            sc_request_screen_capture_access();
        });
    });
}

// ---------------------------------------------------------------------------
// PLMenuHandler — handles native context menu actions
// ---------------------------------------------------------------------------
@interface PLMenuHandler : NSObject
+ (instancetype)shared;
- (void)grantPermission:(id)sender;
- (void)quitApp:(id)sender;
@end

@implementation PLMenuHandler
+ (instancetype)shared {
    static PLMenuHandler* instance = nil;
    static dispatch_once_t token;
    dispatch_once(&token, ^{ instance = [[PLMenuHandler alloc] init]; });
    return instance;
}
- (void)toggleWindow:(id)sender {
    if (g_tray_left_click_cb) g_tray_left_click_cb();
}
- (void)grantPermission:(id)sender {
    sc_show_picker();
}
- (void)quitApp:(id)sender {
    [NSApp terminate:nil];
}
@end

// ---------------------------------------------------------------------------
// sc_show_tray_menu — unified tray menu (left and right click)
//
// Merges window-toggle and screen-capture-permission into one menu so that
// MenuBarDockX (and other tray proxies) can access all actions via a single
// click, regardless of left/right click forwarding capability.
//
// The toggle item title reflects the current window visibility state so the
// user always knows what will happen before clicking.
// ---------------------------------------------------------------------------
void sc_show_tray_menu(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        // Detect whether the PixelLens window is currently visible.
        BOOL windowVisible = NO;
        for (NSWindow* w in NSApp.windows) {
            // PixelLens window: titled "PixelLens", not a panel/status-bar window.
            if ([w.title isEqualToString:@"PixelLens"] && !w.isMiniaturized && w.isVisible) {
                windowVisible = YES;
                break;
            }
        }

        NSMenu* menu = [[NSMenu alloc] init];
        menu.autoenablesItems = NO;

        // ── Toggle item ──────────────────────────────────────────────────────
        // Label changes based on current window state for clear UX.
        NSString* toggleTitle = windowVisible
            ? @"ウィンドウを非表示"
            : @"ウィンドウを表示";
        NSMenuItem* toggleItem = [[NSMenuItem alloc]
            initWithTitle:toggleTitle
                   action:@selector(toggleWindow:)
            keyEquivalent:@""];
        // Show the global shortcut as a hint (display only, not a real binding here).
        toggleItem.keyEquivalentModifierMask = NSEventModifierFlagControl | NSEventModifierFlagOption;
        toggleItem.target  = [PLMenuHandler shared];
        toggleItem.enabled = YES;
        // Checkmark when visible so the user can see the current state at a glance.
        toggleItem.state = windowVisible ? NSControlStateValueOn : NSControlStateValueOff;
        [menu addItem:toggleItem];

        [menu addItem:[NSMenuItem separatorItem]];

        // ── Screen recording permission ──────────────────────────────────────
        NSMenuItem* permItem = [[NSMenuItem alloc]
            initWithTitle:@"画面収録を許可（クリックして選択）"
                   action:@selector(grantPermission:)
            keyEquivalent:@""];
        permItem.target  = [PLMenuHandler shared];
        permItem.enabled = YES;
        [menu addItem:permItem];

        [menu addItem:[NSMenuItem separatorItem]];

        // ── Quit ────────────────────────────────────────────────────────────
        NSMenuItem* quitItem = [[NSMenuItem alloc]
            initWithTitle:@"PixelLens を終了"
                   action:@selector(quitApp:)
            keyEquivalent:@""];
        quitItem.target  = [PLMenuHandler shared];
        quitItem.enabled = YES;
        [menu addItem:quitItem];

        // Pop up at current cursor position.
        NSPoint pos = [NSEvent mouseLocation];
        [menu popUpMenuPositioningItem:nil atLocation:pos inView:nil];
    });
}

// Keep old name as an alias for any remaining callers.
void sc_show_context_menu(void) { sc_show_tray_menu(); }

bool sc_has_screen_capture_permission(void) {
    return CGPreflightScreenCaptureAccess();
}
