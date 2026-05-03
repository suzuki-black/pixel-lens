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

/// Display geometry stored at stream-start time.
/// logical_h: CG logical height (for Y-flip from CG coords to buffer coords).
/// scale:     backingScaleFactor (logical → physical pixel conversion).
static double g_display_logical_h = 0.0;
static double g_display_scale     = 1.0;

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

- (void)contentSharingPicker:(SCContentSharingPicker*)picker
        didUpdateWithFilter:(SCContentFilter*)filter
                 forStream:(SCStream* _Nullable)stream
{
    pl_log("Picker: didUpdateWithFilter — starting SCStream with picker filter");

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

    // Capture display geometry for coordinate mapping.
    NSScreen* ms = [NSScreen mainScreen];
    double scale  = ms ? (double)ms.backingScaleFactor : 2.0;
    double logW   = ms ? ms.frame.size.width  : 1920.0;
    double logH   = ms ? ms.frame.size.height : 1080.0;
    size_t physW  = (size_t)(logW * scale + 0.5);
    size_t physH  = (size_t)(logH * scale + 0.5);
    if (physW < 1) physW = 1;
    if (physH < 1) physH = 1;

    pl_log("Stream: NSScreen=%.0fx%.0f bsf=%.1f → buf=%zux%zu",
           logW, logH, scale, physW, physH);

    // Configure stream for full-display capture at native resolution.
    SCStreamConfiguration* config = [[SCStreamConfiguration alloc] init];
    config.width           = physW;
    config.height          = physH;
    config.pixelFormat     = kCVPixelFormatType_32BGRA;
    config.showsCursor     = NO;
    // 30 fps is plenty for a pixel color picker.
    config.minimumFrameInterval = CMTimeMake(1, 30);

    // Lazy-init singletons.
    if (!g_sample_lock)    g_sample_lock    = [[NSLock alloc] init];
    if (!g_sample_queue)   g_sample_queue   = dispatch_queue_create("dev.pixellens.stream", nil);
    if (!g_stream_output)  g_stream_output  = [[PLStreamOutput alloc] init];
    if (!g_stream_delegate) g_stream_delegate = [[PLStreamDelegate alloc] init];

    g_display_logical_h = logH;
    g_display_scale     = scale;
    g_no_auth_logged    = NO;

    // Create the stream.
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
            pl_log("Stream: started OK buf=%zux%zu logH=%.0f scale=%.1f",
                   physW, physH, logH, scale);
        }
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
// release_crop_buf — CGDataProvider release callback
// ---------------------------------------------------------------------------
static void release_crop_buf(void* info, const void* data, size_t size) {
    free((void*)data);
}

// ---------------------------------------------------------------------------
// try_sckit_capture — crop from latest SCStream frame
//
// Parameters (all in CG logical coordinates, origin bottom-left):
//   x, y  — top-left corner of capture rect (NOTE: y is CG bottom in CG convention)
//   w, h  — size in logical pixels
// ---------------------------------------------------------------------------
static CGImageRef try_sckit_capture(double x, double y, double w, double h)
{
    if (!g_stream || !g_stream_started) {
        if (!g_no_auth_logged) {
            pl_log("SCKit: stream not running — use sc_show_picker() to authorize");
            g_no_auth_logged = YES;
        }
        return nil;
    }

    // Grab the latest frame under the lock.
    [g_sample_lock lock];
    CMSampleBufferRef sample = g_latest_sample
        ? (CMSampleBufferRef)CFRetain(g_latest_sample)
        : NULL;
    [g_sample_lock unlock];

    if (!sample) {
        // Stream started but no frame delivered yet — transient, just skip.
        return nil;
    }

    CVImageBufferRef imgBuf = CMSampleBufferGetImageBuffer(sample);
    if (!imgBuf) { CFRelease(sample); return nil; }

    CVPixelBufferLockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);

    size_t   bufW        = CVPixelBufferGetWidth(imgBuf);
    size_t   bufH        = CVPixelBufferGetHeight(imgBuf);
    size_t   bytesPerRow = CVPixelBufferGetBytesPerRow(imgBuf);
    uint8_t* baseAddr    = (uint8_t*)CVPixelBufferGetBaseAddress(imgBuf);

    // --- Coordinate mapping: CG logical → buffer pixel ---
    //
    // CG coordinate system: origin at BOTTOM-LEFT, y increases upward.
    // Buffer coordinate system: origin at TOP-LEFT, y increases downward.
    //
    // Given:
    //   logH  = logical display height (stored at stream-start)
    //   scale = backingScaleFactor
    //   (x, y, w, h) in CG logical coords where y is the BOTTOM of the rect
    //
    // Buffer pixel of the TOP-LEFT of the capture rect:
    //   buf_x = x * scale
    //   buf_y = (logH - y - h) * scale       ← Y-flip
    //
    double logH  = (g_display_logical_h > 0) ? g_display_logical_h : (double)bufH;
    double scale = (g_display_scale     > 0) ? g_display_scale     : 1.0;

    double buf_x_f = x * scale;
    double buf_y_f = (logH - y - h) * scale;   // Y-flip
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

    pl_log("SCKit: buf=%zux%zu scale=%.1f logH=%.0f "
           "CG=(%.0f,%.0f,%.0f,%.0f) → crop=(%zu,%zu,%zu,%zu)",
           bufW, bufH, scale, logH, x, y, w, h, bx, by, bw, bh);

    if (bw == 0 || bh == 0) {
        pl_log("SCKit: crop region empty after clamping");
        CVPixelBufferUnlockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);
        CFRelease(sample);
        return nil;
    }

    // Copy the cropped BGRA rows into a new buffer.
    size_t   cropStride = bw * 4;
    uint8_t* cropBuf    = (uint8_t*)malloc(cropStride * bh);
    if (!cropBuf) {
        CVPixelBufferUnlockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);
        CFRelease(sample);
        return nil;
    }

    for (size_t row = 0; row < bh; row++) {
        memcpy(cropBuf + row * cropStride,
               baseAddr + (by + row) * bytesPerRow + bx * 4,
               cropStride);
    }

    CVPixelBufferUnlockBaseAddress(imgBuf, kCVPixelBufferLock_ReadOnly);
    CFRelease(sample);

    // Wrap cropBuf in a CGImage (BGRA format).
    // The data provider takes ownership via release_crop_buf.
    CGDataProviderRef dp = CGDataProviderCreateWithData(
        NULL, cropBuf, cropStride * bh, release_crop_buf);
    CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();

    CGImageRef img = CGImageCreate(
        bw, bh, 8, 32, cropStride, cs,
        kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst,
        dp, NULL, false, kCGRenderingIntentDefault);

    CGDataProviderRelease(dp);
    CGColorSpaceRelease(cs);
    return img;   // cropBuf freed when img is released via release_crop_buf
}

// ---------------------------------------------------------------------------
// image_to_rgba: CGImageRef (BGRA) → malloc'd RGBA buffer
// ---------------------------------------------------------------------------
static uint8_t* image_to_rgba(CGImageRef image,
                               uint32_t* out_pixel_width,
                               uint32_t* out_pixel_height)
{
    size_t img_w = CGImageGetWidth(image);
    size_t img_h = CGImageGetHeight(image);
    if (img_w == 0 || img_h == 0) {
        pl_log("image_to_rgba: empty image (%zux%zu)", img_w, img_h);
        return NULL;
    }
    *out_pixel_width  = (uint32_t)img_w;
    *out_pixel_height = (uint32_t)img_h;

    size_t stride = img_w * 4;
    uint8_t* buf = (uint8_t*)malloc(stride * img_h);
    if (!buf) return NULL;

    CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
    CGContextRef ctx = CGBitmapContextCreate(
        buf, img_w, img_h, 8, stride, cs,
        (CGBitmapInfo)(kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst));
    CGColorSpaceRelease(cs);
    if (!ctx) { free(buf); return NULL; }

    CGContextDrawImage(ctx, CGRectMake(0, 0, (CGFloat)img_w, (CGFloat)img_h), image);
    CGContextRelease(ctx);

    // BGRA → RGBA, alpha = 255 (strip premultiplied alpha)
    for (size_t i = 0; i < img_w * img_h * 4; i += 4) {
        uint8_t tmp = buf[i]; buf[i] = buf[i+2]; buf[i+2] = tmp; buf[i+3] = 255;
    }
    return buf;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

uint8_t* sc_capture_rect_rgba(
    double x, double y, double w, double h,
    uint32_t exclude_win_id,
    uint32_t* out_pixel_width,
    uint32_t* out_pixel_height)
{
    (void)exclude_win_id;   // unused; SCStream captures the selected display

    CGImageRef image = try_sckit_capture(x, y, w, h);
    if (!image) return NULL;

    uint8_t* buf = image_to_rgba(image, out_pixel_width, out_pixel_height);
    CGImageRelease(image);
    return buf;
}

void sc_free_buffer(uint8_t* buf) {
    free(buf);
}

bool sc_has_screen_capture_permission(void) {
    return CGPreflightScreenCaptureAccess();
}
