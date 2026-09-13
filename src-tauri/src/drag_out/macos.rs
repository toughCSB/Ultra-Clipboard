use std::ffi::c_void;
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApp, NSAttributedStringNSStringDrawing, NSBezierPath, NSColor, NSDraggingContext,
    NSDraggingItem, NSDraggingSession, NSDraggingSource, NSEvent, NSEventModifierFlags,
    NSEventType, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSImage,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSPasteboardItem, NSView,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableArray, NSPoint, NSRect, NSSize, NSString, NSURL,
};
use tauri::WebviewWindow;

use crate::core::{AppError, Result};

const POINT_SIZE: f64 = 128.0;

const UTI_UTF8_PLAIN_TEXT: &str = "public.utf8-plain-text";

const UTI_HTML: &str = "public.html";

const UTI_RTF: &str = "public.rtf";

type OnDropCallback = Box<dyn Fn(DragResult) + Send>;

#[derive(Debug, Clone, Copy)]
pub enum DragResult {
    Dropped,
    Cancel,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "UltraClipboardDragSource"]
    #[ivars = DragSourceIvars]
    struct DragSource;

    unsafe impl NSObjectProtocol for DragSource {}

    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        unsafe fn dragging_session(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> objc2_app_kit::NSDragOperation {
            objc2_app_kit::NSDragOperation::Copy
        }

        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        unsafe fn dragging_session_end(
            &self,
            _session: &NSDraggingSession,
            _ended_at_point: NSPoint,
            operation: objc2_app_kit::NSDragOperation,
        ) {
            let callback = &self.ivars().on_drop;
            if operation == objc2_app_kit::NSDragOperation::None {
                (callback)(DragResult::Cancel);
            } else {
                (callback)(DragResult::Dropped);
            }
        }
    }
);

struct DragSourceIvars {
    on_drop: OnDropCallback,
}

impl DragSource {
    fn new<F: Fn(DragResult) + Send + 'static>(
        on_drop: F,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DragSourceIvars {
            on_drop: Box::new(on_drop),
        });
        unsafe { msg_send![super(this), init] }
    }
}

pub fn start_drag_files<F: Fn(DragResult) + Send + 'static>(
    window: &WebviewWindow,
    paths: Vec<PathBuf>,
    preview_png: Option<Vec<u8>>,
    on_drop: F,
) -> Result<()> {
    if paths.is_empty() {
        return Err(AppError::Clipboard("drag-out: empty path list".to_string()));
    }

    unsafe {
        let img = build_preview_image(preview_png, paths.first())?;

        let dragging_items = NSMutableArray::new();
        for path in &paths {
            let nsurl = NSURL::fileURLWithPath_isDirectory(
                &NSString::from_str(&path.display().to_string()),
                false,
            );
            let item = NSDraggingItem::initWithPasteboardWriter(
                NSDraggingItem::alloc(),
                &ProtocolObject::from_retained(nsurl),
            );
            dragging_items.addObject(&*item);
        }

        begin_drag_session(window, &dragging_items, &img, on_drop)
    }
}

pub fn start_drag_text<F: Fn(DragResult) + Send + 'static>(
    window: &WebviewWindow,
    plain: String,
    html: Option<String>,
    rtf: Option<String>,
    preview_png: Option<Vec<u8>>,
    on_drop: F,
) -> Result<()> {
    if plain.is_empty() {
        return Err(AppError::Clipboard("drag-out: empty text".to_string()));
    }

    unsafe {
        let img = build_preview_image(preview_png, None)
            .unwrap_or_else(|_| render_text_preview_image(&plain));

        let pb_item = NSPasteboardItem::new();
        let _ = pb_item.setString_forType(
            &NSString::from_str(&plain),
            &NSString::from_str(UTI_UTF8_PLAIN_TEXT),
        );
        if let Some(html) = html.as_deref() {
            let _ =
                pb_item.setString_forType(&NSString::from_str(html), &NSString::from_str(UTI_HTML));
        }
        if let Some(rtf) = rtf.as_deref() {
            let _ =
                pb_item.setString_forType(&NSString::from_str(rtf), &NSString::from_str(UTI_RTF));
        }

        let dragging_items = NSMutableArray::new();
        let item = NSDraggingItem::initWithPasteboardWriter(
            NSDraggingItem::alloc(),
            &ProtocolObject::from_retained(pb_item),
        );
        dragging_items.addObject(&*item);

        begin_drag_session(window, &dragging_items, &img, on_drop)
    }
}

unsafe fn begin_drag_session<F: Fn(DragResult) + Send + 'static>(
    window: &WebviewWindow,
    dragging_items: &NSMutableArray<NSDraggingItem>,
    img: &NSImage,
    on_drop: F,
) -> Result<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| AppError::Clipboard("start_drag must run on main thread".to_string()))?;

    let ns_view_ptr = window
        .ns_view()
        .map_err(|err| AppError::Clipboard(format!("get ns_view failed: {err}")))?;
    let ns_view = &*(ns_view_ptr as *const c_void as *const NSView);
    let ns_window = ns_view
        .window()
        .ok_or_else(|| AppError::Clipboard("ns_view has no window".to_string()))?;
    let content_view = ns_window
        .contentView()
        .ok_or_else(|| AppError::Clipboard("ns_window has no contentView".to_string()))?;

    let cursor: NSPoint = ns_window.mouseLocationOutsideOfEventStream();

    let raw = img.size();
    let (disp_w, disp_h) = if raw.width > 0.0 && raw.height > 0.0 {
        let longest = raw.width.max(raw.height);
        let scale = POINT_SIZE / longest;
        (raw.width * scale, raw.height * scale)
    } else {
        (POINT_SIZE, POINT_SIZE)
    };
    img.setSize(NSSize::new(disp_w, disp_h));

    let image_rect = NSRect::new(
        NSPoint::new(cursor.x - disp_w / 2.0, cursor.y - disp_h / 2.0),
        NSSize::new(disp_w, disp_h),
    );
    for i in 0..dragging_items.count() {
        let item = dragging_items.objectAtIndex(i);
        item.setDraggingFrame_contents(image_rect, Some(img));
    }

    let current_event = NSApp(mtm).currentEvent();
    let timestamp = current_event.map(|e| e.timestamp()).unwrap_or(0.0);
    let window_number = ns_window.windowNumber();

    let drag_event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDragged,
        cursor,
        NSEventModifierFlags::empty(),
        timestamp,
        window_number,
        None,
        0,
        1,
        1.0,
    ).ok_or_else(|| AppError::Clipboard("create NSEvent failed".to_string()))?;

    let source = DragSource::new(on_drop, mtm);

    let _ = content_view.beginDraggingSessionWithItems_event_source(
        dragging_items,
        &drag_event,
        &ProtocolObject::<dyn NSDraggingSource>::from_retained(source),
    );

    Ok(())
}

unsafe fn build_preview_image(
    preview_png: Option<Vec<u8>>,
    fallback_path: Option<&PathBuf>,
) -> Result<Retained<NSImage>> {
    if let Some(bytes) = preview_png {
        let data = objc2_foundation::NSData::from_vec(bytes);
        if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
            return Ok(img);
        }
    }

    let path =
        fallback_path.ok_or_else(|| AppError::Clipboard("no preview image source".to_string()))?;
    NSImage::initByReferencingFile(
        NSImage::alloc(),
        &NSString::from_str(&path.to_string_lossy()),
    )
    .ok_or_else(|| AppError::Clipboard("NSImage init failed".to_string()))
}

const TEXT_PREVIEW_PT: f64 = POINT_SIZE * 2.0;

const TEXT_PREVIEW_MAX_CHARS: usize = 280;

#[allow(deprecated)]
unsafe fn render_text_preview_image(text: &str) -> Retained<NSImage> {
    let size = NSSize::new(TEXT_PREVIEW_PT, TEXT_PREVIEW_PT);
    let img = NSImage::initWithSize(NSImage::alloc(), size);

    let snippet = clamp_text(text, TEXT_PREVIEW_MAX_CHARS);
    let ns_text = NSString::from_str(&snippet);

    let font = NSFont::systemFontOfSize(20.0);
    let color = NSColor::labelColor();
    let para = NSMutableParagraphStyle::new();
    para.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByWordWrapping);

    let keys: [&objc2_foundation::NSAttributedStringKey; 3] = [
        NSFontAttributeName,
        NSForegroundColorAttributeName,
        NSParagraphStyleAttributeName,
    ];
    let values: [Retained<objc2::runtime::AnyObject>; 3] = [
        Retained::cast_unchecked(font),
        Retained::cast_unchecked(color),
        Retained::cast_unchecked(para),
    ];
    let attrs = NSDictionary::from_retained_objects(&keys, &values);

    let attr_str = NSAttributedString::initWithString_attributes(
        NSAttributedString::alloc(),
        &ns_text,
        Some(&attrs),
    );

    img.lockFocus();

    let full_rect = NSRect::new(NSPoint::ZERO, size);
    let corner_radius = 20.0;
    let clip_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
        full_rect,
        corner_radius,
        corner_radius,
    );
    clip_path.addClip();

    let bg = NSColor::textBackgroundColor();
    bg.setFill();
    clip_path.fill();

    let padding = 16.0;
    let text_rect = NSRect::new(
        NSPoint::new(padding, padding),
        NSSize::new(size.width - padding * 2.0, size.height - padding * 2.0),
    );
    attr_str.drawInRect(text_rect);

    img.unlockFocus();

    img
}

fn clamp_text(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut last_was_newline = false;
    for ch in text.chars() {
        if out.chars().count() >= max_chars {
            out.push('…');
            break;
        }
        if ch == '\n' {
            if last_was_newline {
                continue;
            }
            last_was_newline = true;
            out.push('\n');
        } else if ch == '\r' {
            continue;
        } else {
            last_was_newline = false;
            out.push(ch);
        }
    }
    out
}
