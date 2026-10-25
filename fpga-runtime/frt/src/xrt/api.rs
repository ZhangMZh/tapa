//! XRT's C API, loaded from `libxrt_coreutil` when a design first runs on
//! hardware or in hardware emulation. Neither libfrt nor the host program
//! links XRT, so software simulation and cosimulation need no XRT install.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::OnceLock;

use crate::error::{FrtError, Result};

/// An opaque XRT handle (device, kernel, run, buffer or graph).
pub type Handle = *mut c_void;

/// `xclBOSyncDirection`.
pub(super) const SYNC_TO_DEVICE: c_int = 0;
pub(super) const SYNC_FROM_DEVICE: c_int = 1;

/// `ert_cmd_state` values a run reports.
pub(super) const RUN_COMPLETED: c_int = 4;
pub(super) const RUN_ERROR: c_int = 5;
pub(super) const RUN_ABORT: c_int = 6;
pub(super) const RUN_SUBMITTED: c_int = 7;
pub(super) const RUN_TIMEOUT: c_int = 8;
pub(super) const RUN_NORESPONSE: c_int = 9;
pub(super) const RUN_SKERROR: c_int = 10;
pub(super) const RUN_SKCRASHED: c_int = 11;

/// The entry points frt uses, resolved once per process.
pub struct Api {
    pub device_open: unsafe extern "C" fn(c_uint) -> Handle,
    pub device_open_by_bdf: unsafe extern "C" fn(*const c_char) -> Handle,
    pub device_close: unsafe extern "C" fn(Handle) -> c_int,
    pub device_load_xclbin_file: unsafe extern "C" fn(Handle, *const c_char) -> c_int,
    pub device_get_xclbin_uuid: unsafe extern "C" fn(Handle, *mut u8) -> c_int,
    pub kernel_open: unsafe extern "C" fn(Handle, *const u8, *const c_char) -> Handle,
    pub kernel_close: unsafe extern "C" fn(Handle) -> c_int,
    pub kernel_arg_group_id: unsafe extern "C" fn(Handle, c_int) -> c_int,
    pub bo_alloc: unsafe extern "C" fn(Handle, usize, u64, u32) -> Handle,
    pub bo_alloc_user_ptr: unsafe extern "C" fn(Handle, *mut c_void, usize, u64, u32) -> Handle,
    pub bo_free: unsafe extern "C" fn(Handle) -> c_int,
    pub bo_write: unsafe extern "C" fn(Handle, *const c_void, usize, usize) -> c_int,
    pub bo_read: unsafe extern "C" fn(Handle, *mut c_void, usize, usize) -> c_int,
    pub bo_sync: unsafe extern "C" fn(Handle, c_int, usize, usize) -> c_int,
    pub run_open: unsafe extern "C" fn(Handle) -> Handle,
    pub run_close: unsafe extern "C" fn(Handle) -> c_int,
    /// Variadic: XRT reads the value by the argument's type in the xclbin
    /// (see [`super::device`]'s scalar lowering).
    pub run_set_arg: unsafe extern "C" fn(Handle, c_int, ...) -> c_int,
    pub run_start: unsafe extern "C" fn(Handle) -> c_int,
    pub run_state: unsafe extern "C" fn(Handle) -> c_int,
    pub run_wait: unsafe extern "C" fn(Handle) -> c_int,
    pub graph_open: unsafe extern "C" fn(Handle, *const u8, *const c_char) -> Handle,
    pub graph_close: unsafe extern "C" fn(Handle),
    pub graph_reset: unsafe extern "C" fn(Handle) -> c_int,
    /// Iterations: `-1` runs forever.
    pub graph_run: unsafe extern "C" fn(Handle, c_int) -> c_int,
    /// Waits `cycles` AI Engine cycles since the run started, then stops
    /// the graph; `0` waits for it to finish.
    pub graph_wait: unsafe extern "C" fn(Handle, u64) -> c_int,
    pub graph_update_rtp:
        unsafe extern "C" fn(Handle, *const c_char, *const c_char, usize) -> c_int,
}

// SAFETY: plain function pointers into a library loaded for the process's
// lifetime, callable from any thread.
unsafe impl Send for Api {}
// SAFETY: as above; the struct is immutable after loading.
unsafe impl Sync for Api {}

/// XRT's core library by its soname.
const LIBRARY: &str = "libxrt_coreutil.so.2";

/// The XRT API, loading `libxrt_coreutil` on first use.
pub fn api() -> Result<&'static Api> {
    static API: OnceLock<std::result::Result<Api, String>> = OnceLock::new();
    API.get_or_init(load)
        .as_ref()
        .map_err(|reason| FrtError::NoXrtRuntime(reason.clone()))
}

/// Where the library is looked for, in order: the install `XILINX_XRT`
/// names, the loader's search path, then XRT's default prefix. XRT's
/// `setup.sh` puts its `lib` on `LD_LIBRARY_PATH`, but that only reaches
/// processes started after it is sourced.
fn candidates() -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(root) = std::env::var_os("XILINX_XRT").filter(|v| !v.is_empty()) {
        let path = std::path::Path::new(&root).join("lib").join(LIBRARY);
        paths.push(path.to_string_lossy().into_owned());
    }
    paths.push(LIBRARY.to_owned());
    paths.push(format!("/opt/xilinx/xrt/lib/{LIBRARY}"));
    paths
}

fn open_library() -> std::result::Result<*mut c_void, String> {
    let mut reasons = Vec::new();
    for candidate in candidates() {
        let name = CString::new(candidate).map_err(|e| format!("bad library path: {e}"))?;
        // SAFETY: dlopen with a valid C string; the handle is never closed,
        // so every resolved pointer stays valid for the process.
        let lib = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
        if !lib.is_null() {
            return Ok(lib);
        }
        // SAFETY: dlerror returns a C string or null.
        let error = unsafe { libc::dlerror() };
        if !error.is_null() {
            // SAFETY: a non-null dlerror result is a NUL-terminated string.
            reasons.push(
                unsafe { CStr::from_ptr(error) }
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    Err(format!(
        "cannot load {LIBRARY} (is XRT installed and `XILINX_XRT` set?): {}",
        reasons.join("; ")
    ))
}

fn load() -> std::result::Result<Api, String> {
    let lib = open_library()?;
    // Each field's type is the C signature of the XRT symbol of that name
    // (xrt_device.h, xrt_kernel.h, xrt_bo.h, xrt_graph.h).
    macro_rules! sym {
        ($name:literal) => {
            // SAFETY: the field this initializes is typed as `$name`'s C
            // signature.
            unsafe { resolve(lib, $name) }?
        };
    }
    Ok(Api {
        device_open: sym!("xrtDeviceOpen"),
        device_open_by_bdf: sym!("xrtDeviceOpenByBDF"),
        device_close: sym!("xrtDeviceClose"),
        device_load_xclbin_file: sym!("xrtDeviceLoadXclbinFile"),
        device_get_xclbin_uuid: sym!("xrtDeviceGetXclbinUUID"),
        kernel_open: sym!("xrtPLKernelOpen"),
        kernel_close: sym!("xrtKernelClose"),
        kernel_arg_group_id: sym!("xrtKernelArgGroupId"),
        bo_alloc: sym!("xrtBOAlloc"),
        bo_alloc_user_ptr: sym!("xrtBOAllocUserPtr"),
        bo_free: sym!("xrtBOFree"),
        bo_write: sym!("xrtBOWrite"),
        bo_read: sym!("xrtBORead"),
        bo_sync: sym!("xrtBOSync"),
        run_open: sym!("xrtRunOpen"),
        run_close: sym!("xrtRunClose"),
        run_set_arg: sym!("xrtRunSetArg"),
        run_start: sym!("xrtRunStart"),
        run_state: sym!("xrtRunState"),
        run_wait: sym!("xrtRunWait"),
        graph_open: sym!("xrtGraphOpen"),
        graph_close: sym!("xrtGraphClose"),
        graph_reset: sym!("xrtGraphReset"),
        graph_run: sym!("xrtGraphRun"),
        graph_wait: sym!("xrtGraphWait"),
        graph_update_rtp: sym!("xrtGraphUpdateRTP"),
    })
}

/// The `name` symbol of `lib` as a function pointer of type `F`.
///
/// # Safety
/// `F` must be a function pointer type matching the symbol's C signature.
unsafe fn resolve<F: Copy>(lib: *mut c_void, name: &str) -> std::result::Result<F, String> {
    assert_eq!(
        std::mem::size_of::<F>(),
        std::mem::size_of::<*mut c_void>(),
        "`F` must be a function pointer"
    );
    let symbol = CString::new(name).map_err(|e| format!("bad symbol name `{name}`: {e}"))?;
    // SAFETY: lib is a live dlopen handle and symbol a C string.
    let ptr = unsafe { libc::dlsym(lib, symbol.as_ptr()) };
    if ptr.is_null() {
        return Err(format!("{LIBRARY} has no `{name}`"));
    }
    // SAFETY: the caller guarantees `F` is the symbol's function type, and
    // the sizes match (asserted above).
    Ok(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&ptr) })
}
