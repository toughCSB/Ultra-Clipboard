//! Wraps WebView2 drop targets so OLE drag ghost previews are rendered inside
//! this application's window without breaking HTML5 drop handling.

use std::ffi::c_void;
use std::iter::once;
use std::sync::{Mutex, OnceLock};

use tauri::WebviewWindow;
use windows::core::{implement, ComInterface, IUnknown, Interface, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, POINTL};
use windows::Win32::System::Com::{CoCreateInstance, IDataObject, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, RegisterDragDrop, RevokeDragDrop, DROPEFFECT,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{CLSID_DragDropHelper, IDropTargetHelper};
use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetPropW};

const PROP_DROP_TARGET: &str = "OleDropTargetInterface";

#[implement(IDropTarget)]
struct ForwardingDropTarget {
    inner: IDropTarget,
    helper: IDropTargetHelper,
    hwnd: HWND,
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for ForwardingDropTarget {
    fn DragEnter(
        &self,
        pdataobj: Option<&IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            let p = POINT { x: pt.x, y: pt.y };
            let effect = *pdweffect;
            let _ = self.helper.DragEnter(self.hwnd, pdataobj, &p, effect);

            self.inner.DragEnter(pdataobj, grfkeystate, *pt, pdweffect)
        }
    }

    fn DragOver(
        &self,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            let p = POINT { x: pt.x, y: pt.y };
            let effect = *pdweffect;
            let _ = self.helper.DragOver(&p, effect);

            self.inner.DragOver(grfkeystate, *pt, pdweffect)
        }
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        unsafe {
            let _ = self.helper.DragLeave();

            self.inner.DragLeave()
        }
    }

    fn Drop(
        &self,
        pdataobj: Option<&IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            let p = POINT { x: pt.x, y: pt.y };
            let effect = *pdweffect;
            let _ = self.helper.Drop(pdataobj, &p, effect);

            self.inner.Drop(pdataobj, grfkeystate, *pt, pdweffect)
        }
    }
}

static GHOST_INSTALL_RESULT: Mutex<bool> = Mutex::new(false);

static WRAPPED_INNERS: OnceLock<Mutex<Vec<usize>>> = OnceLock::new();

fn wrapped_inners() -> &'static Mutex<Vec<usize>> {
    WRAPPED_INNERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn install_for_window(window: &WebviewWindow) {
    let Ok(raw_hwnd) = window.hwnd() else {
        return;
    };

    let hwnd = HWND(raw_hwnd.0 as isize);

    unsafe {
        try_wrap_hwnd(hwnd);

        let _ = EnumChildWindows(hwnd, Some(enum_child_proc), LPARAM(0));
    }

    let mut installed = GHOST_INSTALL_RESULT.lock().unwrap();
    if !*installed {
        *installed = true;
        log::debug!("drag ghost: installed on clipboard window tree");
    }
}

extern "system" fn enum_child_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    unsafe {
        try_wrap_hwnd(hwnd);
    }
    BOOL(1)
}

unsafe fn try_wrap_hwnd(hwnd: HWND) {
    let prop_w: Vec<u16> = PROP_DROP_TARGET.encode_utf16().chain(once(0)).collect();
    let handle = GetPropW(hwnd, PCWSTR(prop_w.as_ptr()));
    if handle.is_invalid() || handle.0 == 0 {
        return;
    }

    let raw = handle.0 as *mut c_void;
    if raw.is_null() {
        return;
    }

    {
        let inners = wrapped_inners().lock().unwrap();
        if inners.contains(&(raw as usize)) {
            return;
        }
    }

    let raw_ref = &raw;
    let Some(unk) = IUnknown::from_raw_borrowed(raw_ref) else {
        return;
    };
    let Ok(inner) = unk.cast::<IDropTarget>() else {
        return;
    };

    let helper: IDropTargetHelper =
        match CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER) {
            Ok(h) => h,
            Err(err) => {
                log::warn!("drag ghost: CoCreateInstance(IDropTargetHelper) failed: {err}");
                return;
            }
        };

    let wrapper: IDropTarget = ForwardingDropTarget {
        inner,
        helper,
        hwnd,
    }
    .into();

    let _ = RevokeDragDrop(hwnd);
    if let Err(err) = RegisterDragDrop(hwnd, &wrapper) {
        log::warn!(
            "drag ghost: RegisterDragDrop on hwnd {:?} failed: {err}",
            hwnd.0
        );
        return;
    }

    wrapped_inners().lock().unwrap().push(raw as usize);
    log::debug!("drag ghost: wrapped IDropTarget on hwnd {:?}", hwnd.0);
}
