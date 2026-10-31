use libloading::Library;
use std::ffi::c_int;
use std::os::raw::c_char;
use std::sync::Arc;

type XdoContext = *mut std::ffi::c_void;
type Window = std::ffi::c_ulong;
const CURRENTWINDOW: Window = 0;

type FnXdoNew = unsafe extern "C" fn(display: *const c_char) -> XdoContext;
type FnXdoFree = unsafe extern "C" fn(xdo: XdoContext);
type FnXdoMouseDown = unsafe extern "C" fn(xdo: XdoContext, window: Window, button: c_int) -> c_int;
type FnXdoMouseUp = unsafe extern "C" fn(xdo: XdoContext, window: Window, button: c_int) -> c_int;
type FnXdoMoveMouseRelative = unsafe extern "C" fn(xdo: XdoContext, x: c_int, y: c_int) -> c_int;

/// Wrapper around dynamically loaded libxdo (libxdo.so.4, libxdo.so.3, libxdo.so)
pub struct DynamicXDo {
    _lib: Arc<Library>,
    ctx: XdoContext,
    fn_free: FnXdoFree,
    fn_mouse_down: FnXdoMouseDown,
    fn_mouse_up: FnXdoMouseUp,
    fn_move_mouse_relative: FnXdoMoveMouseRelative,
}

// xdo functions are thread-safe when not used concurrently on the same context
unsafe impl Send for DynamicXDo {}

impl DynamicXDo {
    /// Try loading libxdo from known shared library names
    pub fn new() -> Result<Self, String> {
        let candidates = ["libxdo.so.4", "libxdo.so.3", "libxdo.so"];
        let mut last_err = String::new();

        for candidate in candidates {
            match unsafe { Library::new(candidate) } {
                Ok(lib) => {
                    log::info!("Loaded libxdo dynamically from {}", candidate);
                    let lib = Arc::new(lib);
                    return Self::init_from_lib(lib);
                }
                Err(e) => {
                    last_err = format!("Failed to load {}: {}", candidate, e);
                }
            }
        }

        Err(format!(
            "Could not load libxdo shared library (checked {}): {}",
            candidates.join(", "),
            last_err
        ))
    }

    fn init_from_lib(lib: Arc<Library>) -> Result<Self, String> {
        unsafe {
            let fn_new: FnXdoNew = *lib
                .get(b"xdo_new\0")
                .map_err(|e| format!("Missing xdo_new: {e}"))?;
            let fn_free: FnXdoFree = *lib
                .get(b"xdo_free\0")
                .map_err(|e| format!("Missing xdo_free: {e}"))?;
            let fn_mouse_down: FnXdoMouseDown = *lib
                .get(b"xdo_mouse_down\0")
                .map_err(|e| format!("Missing xdo_mouse_down: {e}"))?;
            let fn_mouse_up: FnXdoMouseUp = *lib
                .get(b"xdo_mouse_up\0")
                .map_err(|e| format!("Missing xdo_mouse_up: {e}"))?;
            let fn_move_mouse_relative: FnXdoMoveMouseRelative = *lib
                .get(b"xdo_move_mouse_relative\0")
                .map_err(|e| format!("Missing xdo_move_mouse_relative: {e}"))?;

            let ctx = fn_new(std::ptr::null());
            if ctx.is_null() {
                return Err("xdo_new returned NULL".to_string());
            }

            Ok(Self {
                _lib: lib,
                ctx,
                fn_free,
                fn_mouse_down,
                fn_mouse_up,
                fn_move_mouse_relative,
            })
        }
    }

    pub fn mouse_down(&self, button: i32) -> Result<(), i32> {
        let ret = unsafe { (self.fn_mouse_down)(self.ctx, CURRENTWINDOW, button as c_int) };
        if ret == 0 {
            Ok(())
        } else {
            Err(ret)
        }
    }

    pub fn mouse_up(&self, button: i32) -> Result<(), i32> {
        let ret = unsafe { (self.fn_mouse_up)(self.ctx, CURRENTWINDOW, button as c_int) };
        if ret == 0 {
            Ok(())
        } else {
            Err(ret)
        }
    }

    pub fn move_mouse_relative(&self, x: i32, y: i32) -> Result<(), i32> {
        let ret = unsafe { (self.fn_move_mouse_relative)(self.ctx, x as c_int, y as c_int) };
        if ret == 0 {
            Ok(())
        } else {
            Err(ret)
        }
    }
}

impl Drop for DynamicXDo {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe {
                (self.fn_free)(self.ctx);
            }
            self.ctx = std::ptr::null_mut();
        }
    }
}
