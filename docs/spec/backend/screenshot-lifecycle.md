# Screenshot Lifecycle

The screenshot module owns a capture session from native capture through
selection, cancellation, or editor handoff. Its identity is reserved before
native work starts, including captures which do not use a selection overlay.

## Session Transitions

- Only one capture or editor handoff is active at a time.
- Session publication, completion, and editor handoff must match the active
  capture identity. Delayed pixels, failures, and timeouts cannot end a newer
  capture.
- Taking a selection session does not release the active capture reservation.
  Cancellation releases it after hiding overlays and restoring the foreground;
  editor handoff releases it after hiding overlays for the matching editor.
- The matching editor may complete its handoff only once. A late readiness or
  close callback from another editor cannot hide the current overlays.

## Window Effects

Capture lifecycle window effects run on Tauri's main thread. Async commands
dispatch reveal, selection commit, cancellation, editor reveal, and close through
`screenshot::on_main_thread` and await their results. Native capture completion
and readiness fallback callbacks use the same main-thread queue. This keeps a
session check and its window effects ordered with cancellation.

The platform display order is preserved: macOS shows overlays before collecting
pixels; Windows prepares the frozen frame before showing overlays. Multiple
monitors and the editor's first-paint handoff remain supported.

On Windows, one monitor captures directly. Multiple monitors capture up to
four disjoint rectangles at a time on separate worker threads, with each task
owning and releasing its GDI contexts on that thread. Results preserve monitor
order. This reduces serial waiting without promising one atomic desktop frame.

On macOS, the capture worker waits at most 10 seconds for the main thread to
show the overlays before asking ScreenCaptureKit for shareable content. The
active session ID is checked again after the wait. The request includes
off-screen windows, because overlays shown a moment earlier may not count as
on screen yet; without them this app drops out of the list and the overlays,
already shaded, would be captured. The prewarmed overlay WebViews use
Tauri's disabled background throttling policy on macOS so a long-hidden page
is not suspended or unloaded before the next capture. Other windows retain
their normal policy. This trades idle WebView memory for repeat-capture latency.

### macOS Capture Coordinates

macOS lays displays out in points, and a 1x display next to a 2x display would
overlap if each origin used its own scale. Capture pixels therefore keep each
display's scale inside the display, but place every display origin at two
pixels per point. Code that crosses into macOS points goes through the backend:
`cursor_monitor` finds the display under the pointer from Core Graphics points,
and `window_frame` places overlays, editors, and pins with logical positions.
Tauri's physical positions are not used on macOS, because Tauri converts them
with the main display's scale or with the scale of the display a window sits on.

The screenshot editor Save action writes the rendered PNG directly to Pictures,
falling back to Downloads. Its name uses the source window title when available,
the source app name on macOS when the title is unavailable, or `Screen`, plus
the capture timestamp. Unsafe filename characters
are replaced and an existing file is never overwritten by automatic Save. Save
As keeps the native dialog and suggests the same name.

Screenshot editors and pinned images reject file URL navigation. A PNG dropped
back onto a screenshot cannot replace the app document with the browser's image
viewer. Native close requests destroy an editor whose app document is absent,
instead of waiting for a frontend close listener that can no longer receive it.

Image text selection analyzes the rendered editor document with the operating
system OCR engine. Rust returns word regions in export-image pixel coordinates
and reading order, without copying text. The frontend maps those regions through
the editor viewport and presents selectable text; selected text is copied through
the screenshot text command. The existing area OCR action still copies its whole
recognized result.

Screenshot translation is an explicit action on the selected area or the rendered
document. OCR runs locally; only recognized text and block IDs are sent to the
OpenAI Responses API. The client requests structured Korean output with
`store: false`, validates response IDs, and bounds input, response size, retries
and total request time. Credentials come from `OPENAI_API_KEY` or the native OS
credential store, with environment credentials taking precedence; status exposes
the source but never the key. Credentials and translation text are not written
to the settings file, clipboard history, or application logs by this operation.

Translation requests carry a positive JavaScript-safe request ID. The translation
module owns the whole request: the export action only hands it the pixels and the
ID. A per-window registry records cancellations even before registration, rejects
duplicate work, and aborts the asynchronous OCR-and-translate task when cancelled
or when the window closes. OCR already running on a blocking worker may finish
locally, but a cancelled operation does not start its subsequent HTTP request.
Cancelling an HTTP request does not undo a request already accepted by the
provider.

A cancelled request is an explicit outcome: the export result sets
`translationCancelled` and carries no `translationBlocks`. Empty
`translationBlocks` always means the image held no text. The frontend ignores a
cancelled result without a message.

The frontend preserves the original document and renders translation blocks
through the same canvas path used by copy, save, pin and drag exports. Overflow
is visibly marked and full source/translation text remains available separately.
Exporting the translated variant does not mark unsaved original edits as saved;
cancelling a Save As dialog does not mark either variant as exported.

Translation text keeps the dominant source ink color when the surrounding
background is uniform and the source pixels provide a reliable color sample.
Antialiased edges are excluded from the dominant ink estimate. Mixed colors,
textured backgrounds and insufficient samples use contrasting dark or light text.

## Verification

`cargo test screenshot:: --lib` covers stale identities, cancelled pixels,
matching and repeated editor handoffs, and cancellation effects through the
same helper used by production cancellation. Assertions include whether hide
and foreground restoration occur, not just the final session value.

Native capture, focus restoration, and first-paint display still require a
desktop check on each supported platform; state tests do not prove native
window rendering or macOS capture permission behavior.
