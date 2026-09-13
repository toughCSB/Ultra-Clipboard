//! Windows drag-out uses the `drag` crate for files and a custom `IDataObject`
//! for plain text, HTML, and RTF.

use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Once, OnceLock};

use tauri::WebviewWindow;
use windows::core::{implement, Error as WinError, HRESULT, HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    BOOL, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC,
    E_NOTIMPL, OLE_E_ADVISENOTSUPPORTED, S_OK,
};
use windows::Win32::System::Com::{
    IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DVASPECT_CONTENT,
    FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_FIXED,
};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, OleInitialize, ReleaseStgMedium, CF_UNICODETEXT,
    DROPEFFECT, DROPEFFECT_COPY,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};

use crate::core::{AppError, Result};

static OLE_INIT: Once = Once::new();
static CF_HTML: OnceLock<u16> = OnceLock::new();
static CF_RTF: OnceLock<u16> = OnceLock::new();

fn ensure_ole_init() {
    OLE_INIT.call_once(|| unsafe {
        let _ = OleInitialize(Some(std::ptr::null_mut()));
    });
}

fn register_format(name: &str, slot: &'static OnceLock<u16>) -> u16 {
    *slot.get_or_init(|| {
        let wide: Vec<u16> = std::ffi::OsStr::new(name)
            .encode_wide()
            .chain(once(0))
            .collect();
        let id = unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) };

        id as u16
    })
}

fn cf_html() -> u16 {
    register_format("HTML Format", &CF_HTML)
}

fn cf_rtf() -> u16 {
    register_format("Rich Text Format", &CF_RTF)
}

fn build_cf_html_payload(html: &str) -> Vec<u8> {
    const HEADER_TMPL: &str = "Version:0.9\r\n\
        StartHTML:0000000000\r\n\
        EndHTML:0000000000\r\n\
        StartFragment:0000000000\r\n\
        EndFragment:0000000000\r\n";
    const HTML_PREFIX: &str = "<html><body>\r\n<!--StartFragment-->";
    const HTML_SUFFIX: &str = "<!--EndFragment-->\r\n</body></html>";

    let mut buf = String::with_capacity(
        HEADER_TMPL.len() + HTML_PREFIX.len() + html.len() + HTML_SUFFIX.len(),
    );
    buf.push_str(HEADER_TMPL);
    let start_html = buf.len();
    buf.push_str(HTML_PREFIX);
    let start_fragment = buf.len();
    buf.push_str(html);
    let end_fragment = buf.len();
    buf.push_str(HTML_SUFFIX);
    let end_html = buf.len();

    let patch = |buf: &mut String, label: &str, value: usize| {
        let needle = format!("{label}:0000000000");
        let replacement = format!("{label}:{value:010}");
        if let Some(pos) = buf.find(&needle) {
            buf.replace_range(pos..pos + needle.len(), &replacement);
        }
    };
    patch(&mut buf, "StartHTML", start_html);
    patch(&mut buf, "EndHTML", end_html);
    patch(&mut buf, "StartFragment", start_fragment);
    patch(&mut buf, "EndFragment", end_fragment);

    buf.into_bytes()
}

fn bytes_to_stgmedium(bytes: &[u8]) -> windows::core::Result<STGMEDIUM> {
    unsafe {
        let handle = GlobalAlloc(GMEM_FIXED, bytes.len())?;
        let ptr = GlobalLock(handle) as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(handle);

        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: handle },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        })
    }
}

unsafe fn clone_hglobal_medium(src: &STGMEDIUM) -> windows::core::Result<STGMEDIUM> {
    let src_handle = src.u.hGlobal;
    let size = GlobalSize(src_handle);
    let src_ptr = GlobalLock(src_handle) as *const u8;
    if src_ptr.is_null() {
        return Err(WinError::new(E_NOTIMPL, HSTRING::new()));
    }

    let new_handle = match GlobalAlloc(GMEM_FIXED, size) {
        Ok(h) => h,
        Err(err) => {
            let _ = GlobalUnlock(src_handle);
            return Err(err);
        }
    };
    let dst_ptr = GlobalLock(new_handle) as *mut u8;
    std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, size);
    let _ = GlobalUnlock(new_handle);
    let _ = GlobalUnlock(src_handle);

    Ok(STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 {
            hGlobal: new_handle,
        },
        pUnkForRelease: std::mem::ManuallyDrop::new(None),
    })
}

struct StoredEntry {
    format: FORMATETC,
    medium: STGMEDIUM,
}

unsafe impl Send for StoredEntry {}
unsafe impl Sync for StoredEntry {}

impl Drop for StoredEntry {
    fn drop(&mut self) {
        unsafe {
            ReleaseStgMedium(&mut self.medium);
        }
    }
}

#[implement(IDataObject)]
struct RichDataObject {
    text_utf16: Vec<u16>,

    html_bytes: Option<Vec<u8>>,

    rtf_bytes: Option<Vec<u8>>,

    extras: std::sync::Mutex<Vec<StoredEntry>>,
}

impl RichDataObject {
    fn new(plain: &str, html: Option<&str>, rtf: Option<&str>) -> Self {
        let text_utf16: Vec<u16> = std::ffi::OsStr::new(plain)
            .encode_wide()
            .chain(once(0))
            .collect();
        Self {
            text_utf16,
            html_bytes: html.map(build_cf_html_payload),
            rtf_bytes: rtf.map(|s| s.as_bytes().to_vec()),
            extras: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn supported_format(&self, format: *const FORMATETC) -> Option<u16> {
        let fmt = unsafe { format.as_ref()? };
        if fmt.tymed as i32 != TYMED_HGLOBAL.0 || fmt.dwAspect != DVASPECT_CONTENT.0 {
            return None;
        }
        if fmt.cfFormat == CF_UNICODETEXT.0 {
            return Some(CF_UNICODETEXT.0);
        }
        if self.html_bytes.is_some() && fmt.cfFormat == cf_html() {
            return Some(fmt.cfFormat);
        }
        if self.rtf_bytes.is_some() && fmt.cfFormat == cf_rtf() {
            return Some(fmt.cfFormat);
        }
        None
    }

    fn alloc_for(&self, cf: u16) -> windows::core::Result<STGMEDIUM> {
        if cf == CF_UNICODETEXT.0 {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    self.text_utf16.as_ptr() as *const u8,
                    self.text_utf16.len() * std::mem::size_of::<u16>(),
                )
            };
            return bytes_to_stgmedium(bytes);
        }
        if cf == cf_html() {
            if let Some(b) = &self.html_bytes {
                return bytes_to_stgmedium(b);
            }
        }
        if cf == cf_rtf() {
            if let Some(b) = &self.rtf_bytes {
                return bytes_to_stgmedium(b);
            }
        }
        Err(WinError::new(DV_E_FORMATETC, HSTRING::new()))
    }
}

#[allow(non_snake_case)]
impl IDataObject_Impl for RichDataObject {
    fn GetData(&self, pformatetc: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        if let Some(cf) = self.supported_format(pformatetc) {
            return self.alloc_for(cf);
        }

        let req = unsafe {
            match pformatetc.as_ref() {
                Some(f) => *f,
                None => return Err(WinError::new(DV_E_FORMATETC, HSTRING::new())),
            }
        };
        let extras = self
            .extras
            .lock()
            .map_err(|_| WinError::new(E_NOTIMPL, HSTRING::new()))?;
        for entry in extras.iter() {
            if entry.format.cfFormat == req.cfFormat
                && (entry.format.tymed & req.tymed) != 0
                && entry.format.dwAspect == req.dwAspect
            {
                if entry.medium.tymed == TYMED_HGLOBAL.0 as u32 {
                    return unsafe { clone_hglobal_medium(&entry.medium) };
                }
                return Err(WinError::new(DV_E_FORMATETC, HSTRING::new()));
            }
        }
        Err(WinError::new(DV_E_FORMATETC, HSTRING::new()))
    }

    fn GetDataHere(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        Err(WinError::new(E_NOTIMPL, HSTRING::new()))
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        if self.supported_format(pformatetc).is_some() {
            return S_OK;
        }
        let Some(req) = (unsafe { pformatetc.as_ref() }) else {
            return DV_E_FORMATETC;
        };
        if let Ok(extras) = self.extras.lock() {
            for entry in extras.iter() {
                if entry.format.cfFormat == req.cfFormat && entry.format.dwAspect == req.dwAspect {
                    return S_OK;
                }
            }
        }
        DV_E_FORMATETC
    }

    fn GetCanonicalFormatEtc(
        &self,
        _pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        unsafe { (*pformatetcout).ptd = std::ptr::null_mut() };
        E_NOTIMPL
    }

    fn SetData(
        &self,
        pformatetc: *const FORMATETC,
        pmedium: *const STGMEDIUM,
        frelease: BOOL,
    ) -> windows::core::Result<()> {
        if !frelease.as_bool() {
            return Err(WinError::new(E_NOTIMPL, HSTRING::new()));
        }
        let format = unsafe {
            match pformatetc.as_ref() {
                Some(f) => *f,
                None => return Err(WinError::new(E_NOTIMPL, HSTRING::new())),
            }
        };
        let medium = unsafe {
            match pmedium.as_ref() {
                Some(m) => std::ptr::read(m),
                None => return Err(WinError::new(E_NOTIMPL, HSTRING::new())),
            }
        };
        let mut extras = self
            .extras
            .lock()
            .map_err(|_| WinError::new(E_NOTIMPL, HSTRING::new()))?;

        extras.retain(|e| {
            !(e.format.cfFormat == format.cfFormat && e.format.dwAspect == format.dwAspect)
        });
        extras.push(StoredEntry { format, medium });
        Ok(())
    }

    fn EnumFormatEtc(&self, _dwdirection: u32) -> windows::core::Result<IEnumFORMATETC> {
        Err(WinError::new(E_NOTIMPL, HSTRING::new()))
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: Option<&IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(WinError::new(OLE_E_ADVISENOTSUPPORTED, HSTRING::new()))
    }

    fn DUnadvise(&self, _dwconnection: u32) -> windows::core::Result<()> {
        Err(WinError::new(OLE_E_ADVISENOTSUPPORTED, HSTRING::new()))
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(WinError::new(OLE_E_ADVISENOTSUPPORTED, HSTRING::new()))
    }
}

#[implement(IDropSource)]
struct DropSource;

#[allow(non_snake_case)]
impl IDropSource_Impl for DropSource {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        if fescapepressed.as_bool() {
            DRAGDROP_S_CANCEL
        } else if (grfkeystate & MK_LBUTTON) == MODIFIERKEYS_FLAGS(0) {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

pub fn start_drag_text(
    window: &WebviewWindow,
    plain: &str,
    html: Option<&str>,
    rtf: Option<&str>,
    _preview_png: Option<Vec<u8>>,
) -> Result<()> {
    if plain.is_empty() {
        return Err(AppError::Clipboard("drag-out: empty text".to_string()));
    }

    ensure_ole_init();
    super::windows_ghost::install_for_window(window);

    let data_object: IDataObject = RichDataObject::new(plain, html, rtf).into();
    let drop_source: IDropSource = DropSource.into();

    let mut out = DROPEFFECT::default();
    let hr = unsafe { DoDragDrop(&data_object, &drop_source, DROPEFFECT_COPY, &mut out) };

    if hr == DRAGDROP_S_DROP {
        log::debug!("drag-out text finished: Dropped");
    } else {
        log::debug!("drag-out text finished: {hr:?}");
    }

    Ok(())
}

pub fn start_drag_files(
    window: &WebviewWindow,
    paths: Vec<PathBuf>,
    preview_png: Option<Vec<u8>>,
) -> Result<()> {
    use drag::{DragItem, Image, Options};

    super::windows_ghost::install_for_window(window);

    let image = match preview_png {
        Some(bytes) => Image::Raw(bytes),
        None => Image::File(paths[0].clone()),
    };

    drag::start_drag(
        window,
        DragItem::Files(paths),
        image,
        |result, _cursor| {
            log::debug!("drag-out files finished: {result:?}");
        },
        Options::default(),
    )
    .map_err(|err| AppError::Clipboard(format!("drag-out failed: {err}")))?;

    Ok(())
}
