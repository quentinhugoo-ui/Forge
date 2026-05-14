//! Forge — minimal CUDA driver bindings.
//!
//! Direct FFI to `nvcuda.dll` (Windows) / `libcuda.so` (Linux), bypassing
//! cudarc and the CUDA Toolkit. The only runtime dependency is the
//! NVIDIA driver itself, which ships with every NVIDIA GPU install.
//!
//! ## Why this exists
//!
//! cudarc (the standard Rust CUDA crate) requires:
//! 1. A specific `cuda-XXXXX` feature matching the user's CUDA version
//!    (compile-time mismatch → runtime "GetProcAddress symbol not found")
//! 2. The CUDA Toolkit installed on the user's machine for `nvrtc.dll`
//!    (which is NOT the driver — toolkit is ~3 GB extra)
//!
//! Both fragility points disappear when we skip cudarc and bind directly
//! to the 22 driver entry points we actually use. These have been frozen
//! since CUDA 4.0 (2011) — `cuLaunchKernel`, `cuMemAlloc_v2`,
//! `cuModuleLoadData`, etc. The driver loader at runtime just resolves
//! their addresses via `GetProcAddress` (Windows) / `dlsym` (Unix).
//!
//! ## Architecture
//!
//! ```text
//!  ┌─────────────────────────────────────────────────────────┐
//!  │ Forge build time (developer machine, has CUDA Toolkit) │
//!  │   nvcc cuda/kasm_interpret.cu → kasm.ptx               │
//!  │   include_str!(...) embeds PTX in Forge.exe            │
//!  └─────────────────────────────────────────────────────────┘
//!                              ↓
//!  ┌─────────────────────────────────────────────────────────┐
//!  │ Forge runtime (user machine, just NVIDIA driver)       │
//!  │   1. LoadLibrary("nvcuda.dll")                         │
//!  │   2. GetProcAddress for 22 entry points                │
//!  │   3. cuModuleLoadData(KASM_PTX) — driver JITs to SASS  │
//!  │   4. cuLaunchKernel(...)                               │
//!  │   No nvrtc, no Toolkit, no cudarc.                     │
//!  └─────────────────────────────────────────────────────────┘
//! ```
//!
//! This is also the moonshot pipeline:
//! - Pinned host memory (`cuMemAllocHost_v2`) for 2× PCIe throughput
//! - Multi-stream copy/compute overlap (`cuStreamCreate` + `*Async_v2`)
//! - PTX cached on disk by the driver across runs
//! - One-shot kernel launch — no per-call dispatch overhead

#![cfg(feature = "cuda")]

use std::ffi::{c_char, c_void, CString};
use std::io;
use std::sync::OnceLock;

// Pre-compiled fatbin of the universal KASM interpreter — contains one
// cubin per supported NVIDIA architecture (sm_75/80/86/89/90) plus a
// PTX fallback (compute_90) for unknown future GPUs. The driver picks
// the matching cubin directly — no PTX JIT for any current hardware,
// so PTX version mismatches between Forge build's nvcc and the user's
// driver simply don't happen.
//
// May be empty if the build machine had no nvcc — runtime checks.
pub const KASM_INTERPRET_FATBIN: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/kasm.fatbin"));

// ─── CUDA driver types ──────────────────────────────────────────────
//
// These are opaque handles in CUDA. On x86_64 they're all pointer-sized.
// `CUdevice` is the only exception — it's an `int`.

pub type CUresult = u32;
pub type CUdevice = i32;
pub type CUcontext = *mut c_void;
pub type CUmodule = *mut c_void;
pub type CUfunction = *mut c_void;
pub type CUstream = *mut c_void;
pub type CUdeviceptr = u64;

pub const CUDA_SUCCESS: CUresult = 0;

// ─── Function pointer table ─────────────────────────────────────────
//
// The 22 driver entry points Forge needs. Loaded dynamically from
// nvcuda.dll at first use via `loader::cuda_table()`. All of these have
// existed since CUDA 4.0 (2011) — they don't move, don't disappear,
// don't change ABI.

pub struct CudaTable {
    pub cu_init: unsafe extern "system" fn(flags: u32) -> CUresult,
    pub cu_device_get_count: unsafe extern "system" fn(count: *mut i32) -> CUresult,
    pub cu_device_get: unsafe extern "system" fn(dev: *mut CUdevice, ordinal: i32) -> CUresult,
    pub cu_ctx_create_v2: unsafe extern "system" fn(
        pctx: *mut CUcontext,
        flags: u32,
        dev: CUdevice,
    ) -> CUresult,
    pub cu_ctx_destroy_v2: unsafe extern "system" fn(ctx: CUcontext) -> CUresult,
    pub cu_ctx_set_current: unsafe extern "system" fn(ctx: CUcontext) -> CUresult,
    pub cu_ctx_synchronize: unsafe extern "system" fn() -> CUresult,
    pub cu_module_load_data: unsafe extern "system" fn(
        module: *mut CUmodule,
        image: *const c_void,
    ) -> CUresult,
    pub cu_module_unload: unsafe extern "system" fn(module: CUmodule) -> CUresult,
    pub cu_module_get_function: unsafe extern "system" fn(
        func: *mut CUfunction,
        module: CUmodule,
        name: *const c_char,
    ) -> CUresult,
    pub cu_mem_alloc_v2: unsafe extern "system" fn(
        dptr: *mut CUdeviceptr,
        bytesize: usize,
    ) -> CUresult,
    pub cu_mem_free_v2: unsafe extern "system" fn(dptr: CUdeviceptr) -> CUresult,
    pub cu_mem_alloc_host_v2: unsafe extern "system" fn(
        pp: *mut *mut c_void,
        bytesize: usize,
    ) -> CUresult,
    pub cu_mem_free_host: unsafe extern "system" fn(p: *mut c_void) -> CUresult,
    pub cu_memcpy_h_to_d_v2: unsafe extern "system" fn(
        dst_device: CUdeviceptr,
        src_host: *const c_void,
        byte_count: usize,
    ) -> CUresult,
    pub cu_memcpy_d_to_h_v2: unsafe extern "system" fn(
        dst_host: *mut c_void,
        src_device: CUdeviceptr,
        byte_count: usize,
    ) -> CUresult,
    pub cu_memcpy_h_to_d_async_v2: unsafe extern "system" fn(
        dst_device: CUdeviceptr,
        src_host: *const c_void,
        byte_count: usize,
        stream: CUstream,
    ) -> CUresult,
    pub cu_memcpy_d_to_h_async_v2: unsafe extern "system" fn(
        dst_host: *mut c_void,
        src_device: CUdeviceptr,
        byte_count: usize,
        stream: CUstream,
    ) -> CUresult,
    pub cu_stream_create: unsafe extern "system" fn(
        stream: *mut CUstream,
        flags: u32,
    ) -> CUresult,
    pub cu_stream_destroy_v2: unsafe extern "system" fn(stream: CUstream) -> CUresult,
    pub cu_stream_synchronize: unsafe extern "system" fn(stream: CUstream) -> CUresult,
    pub cu_launch_kernel: unsafe extern "system" fn(
        f: CUfunction,
        grid_dim_x: u32,
        grid_dim_y: u32,
        grid_dim_z: u32,
        block_dim_x: u32,
        block_dim_y: u32,
        block_dim_z: u32,
        shared_mem_bytes: u32,
        stream: CUstream,
        kernel_params: *mut *mut c_void,
        extra: *mut *mut c_void,
    ) -> CUresult,
    pub cu_get_error_string: unsafe extern "system" fn(
        error: CUresult,
        pstr: *mut *const c_char,
    ) -> CUresult,
}

// ─── Dynamic loader ─────────────────────────────────────────────────

#[cfg(windows)]
mod loader_impl {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    type HMODULE = *mut c_void;
    type FARPROC = *mut c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> HMODULE;
        fn GetProcAddress(h_module: HMODULE, lp_proc_name: *const c_char) -> FARPROC;
    }

    pub fn load_cuda_module() -> Result<HMODULE, String> {
        // nvcuda.dll lives in System32, always present on systems with NVIDIA drivers.
        let name: Vec<u16> = std::ffi::OsStr::new("nvcuda.dll")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let h = unsafe { LoadLibraryW(name.as_ptr()) };
        if h.is_null() {
            Err("LoadLibraryW(nvcuda.dll) returned NULL — NVIDIA driver not installed?"
                .to_string())
        } else {
            Ok(h)
        }
    }

    pub fn resolve(module: HMODULE, name: &str) -> Result<FARPROC, String> {
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        let p = unsafe { GetProcAddress(module, cname.as_ptr()) };
        if p.is_null() {
            Err(format!("GetProcAddress({name}) returned NULL"))
        } else {
            Ok(p)
        }
    }
}

#[cfg(unix)]
mod loader_impl {
    use super::*;

    type Handle = *mut c_void;

    extern "C" {
        fn dlopen(filename: *const c_char, flag: i32) -> Handle;
        fn dlsym(handle: Handle, symbol: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: i32 = 2;

    pub fn load_cuda_module() -> Result<Handle, String> {
        for name in &["libcuda.so.1", "libcuda.so"] {
            let cname = CString::new(*name).unwrap();
            let h = unsafe { dlopen(cname.as_ptr(), RTLD_NOW) };
            if !h.is_null() {
                return Ok(h);
            }
        }
        Err("dlopen(libcuda.so) failed — NVIDIA driver not installed?".to_string())
    }

    pub fn resolve(module: Handle, name: &str) -> Result<*mut c_void, String> {
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        let p = unsafe { dlsym(module, cname.as_ptr()) };
        if p.is_null() {
            Err(format!("dlsym({name}) returned NULL"))
        } else {
            Ok(p)
        }
    }
}

/// Resolve a function pointer to the right type. Macro to keep call sites
/// concise and avoid repeating the type signature.
macro_rules! resolve {
    ($module:expr, $name:literal, $ty:ty) => {{
        let p = loader_impl::resolve($module, $name)?;
        unsafe { std::mem::transmute::<_, $ty>(p) }
    }};
}

/// Returns a singleton table of resolved CUDA driver function pointers.
/// Loads `nvcuda.dll` on first call, caches the result. Subsequent calls
/// return the same table.
pub fn cuda_table() -> Result<&'static CudaTable, String> {
    static TABLE: OnceLock<Result<CudaTable, String>> = OnceLock::new();
    let entry = TABLE.get_or_init(|| {
        let module = loader_impl::load_cuda_module()?;
        Ok(CudaTable {
            cu_init: resolve!(module, "cuInit", _),
            cu_device_get_count: resolve!(module, "cuDeviceGetCount", _),
            cu_device_get: resolve!(module, "cuDeviceGet", _),
            cu_ctx_create_v2: resolve!(module, "cuCtxCreate_v2", _),
            cu_ctx_destroy_v2: resolve!(module, "cuCtxDestroy_v2", _),
            cu_ctx_set_current: resolve!(module, "cuCtxSetCurrent", _),
            cu_ctx_synchronize: resolve!(module, "cuCtxSynchronize", _),
            cu_module_load_data: resolve!(module, "cuModuleLoadData", _),
            cu_module_unload: resolve!(module, "cuModuleUnload", _),
            cu_module_get_function: resolve!(module, "cuModuleGetFunction", _),
            cu_mem_alloc_v2: resolve!(module, "cuMemAlloc_v2", _),
            cu_mem_free_v2: resolve!(module, "cuMemFree_v2", _),
            cu_mem_alloc_host_v2: resolve!(module, "cuMemAllocHost_v2", _),
            cu_mem_free_host: resolve!(module, "cuMemFreeHost", _),
            cu_memcpy_h_to_d_v2: resolve!(module, "cuMemcpyHtoD_v2", _),
            cu_memcpy_d_to_h_v2: resolve!(module, "cuMemcpyDtoH_v2", _),
            cu_memcpy_h_to_d_async_v2: resolve!(module, "cuMemcpyHtoDAsync_v2", _),
            cu_memcpy_d_to_h_async_v2: resolve!(module, "cuMemcpyDtoHAsync_v2", _),
            cu_stream_create: resolve!(module, "cuStreamCreate", _),
            cu_stream_destroy_v2: resolve!(module, "cuStreamDestroy_v2", _),
            cu_stream_synchronize: resolve!(module, "cuStreamSynchronize", _),
            cu_launch_kernel: resolve!(module, "cuLaunchKernel", _),
            cu_get_error_string: resolve!(module, "cuGetErrorString", _),
        })
    });
    entry.as_ref().map_err(|e| e.clone())
}

/// Wraps a CUresult into an io::Result with a human-readable error message
/// fetched via cuGetErrorString.
fn check(table: &CudaTable, code: CUresult, ctx: &str) -> io::Result<()> {
    if code == CUDA_SUCCESS {
        return Ok(());
    }
    let mut msg_ptr: *const c_char = std::ptr::null();
    let detail = unsafe {
        let r = (table.cu_get_error_string)(code, &mut msg_ptr);
        if r == CUDA_SUCCESS && !msg_ptr.is_null() {
            std::ffi::CStr::from_ptr(msg_ptr)
                .to_string_lossy()
                .into_owned()
        } else {
            format!("CUDA error {code}")
        }
    };
    Err(io::Error::other(format!("{ctx}: {detail}")))
}

// ─── Rust-friendly runtime ──────────────────────────────────────────
//
// Thin RAII wrappers. Construction = resource allocation (alloc, create
// stream, etc.). Drop = release. Failure to resource-acquire returns an
// io::Error rather than panicking — caller can fall back to CPU.

/// Globally-initialized CUDA — calls cuInit(0) once. Idempotent.
pub fn ensure_cuda_init() -> io::Result<&'static CudaTable> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    let table = cuda_table().map_err(io::Error::other)?;
    let init_result = INIT.get_or_init(|| {
        let r = unsafe { (table.cu_init)(0) };
        if r != CUDA_SUCCESS {
            Err(format!("cuInit(0) returned {r}"))
        } else {
            Ok(())
        }
    });
    init_result.as_ref().map_err(|e| io::Error::other(e.clone()))?;
    Ok(table)
}

/// Owned CUDA context. Created on first use, destroyed on drop. Wraps
/// the most common pattern: one context per device for the lifetime of
/// the runtime.
pub struct Context {
    pub(crate) ctx: CUcontext,
    pub(crate) table: &'static CudaTable,
}

impl Context {
    /// Create a context on the given device (0 = default GPU).
    pub fn new(device_ordinal: i32) -> io::Result<Self> {
        let table = ensure_cuda_init()?;
        let mut device: CUdevice = 0;
        unsafe { check(table, (table.cu_device_get)(&mut device, device_ordinal), "cuDeviceGet")? };
        let mut ctx: CUcontext = std::ptr::null_mut();
        unsafe { check(table, (table.cu_ctx_create_v2)(&mut ctx, 0, device), "cuCtxCreate_v2")? };
        Ok(Self { ctx, table })
    }

    /// Make this context current for the calling thread.
    pub fn set_current(&self) -> io::Result<()> {
        unsafe { check(self.table, (self.table.cu_ctx_set_current)(self.ctx), "cuCtxSetCurrent") }
    }

    /// Wait for all pending GPU work in this context to complete.
    pub fn synchronize(&self) -> io::Result<()> {
        unsafe { check(self.table, (self.table.cu_ctx_synchronize)(), "cuCtxSynchronize") }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { (self.table.cu_ctx_destroy_v2)(self.ctx) };
        }
    }
}

/// Loaded GPU module — produced by JIT-compiling a PTX string. The
/// driver caches the SASS compilation across runs (per device, per
/// driver version) so the second-onwards launch is instant.
pub struct Module {
    pub(crate) module: CUmodule,
    pub(crate) table: &'static CudaTable,
}

impl Module {
    /// Load a CUDA module from a raw image. Accepts PTX (text), cubin
    /// (binary SASS for a specific arch), or fatbin (universal binary
    /// containing multiple cubins). The driver auto-detects the format.
    pub fn load_image(image: &[u8]) -> io::Result<Self> {
        let table = ensure_cuda_init()?;
        if image.is_empty() {
            return Err(io::Error::other(
                "Forge CUDA kernel image is empty — Forge was built without nvcc on the build \
                 machine; rebuild on a machine with the CUDA Toolkit installed",
            ));
        }
        let mut module: CUmodule = std::ptr::null_mut();
        unsafe {
            check(
                table,
                (table.cu_module_load_data)(&mut module, image.as_ptr() as *const c_void),
                "cuModuleLoadData",
            )?
        };
        Ok(Self { module, table })
    }

    /// Look up a kernel function by its `__global__` symbol name.
    pub fn get_function(&self, name: &str) -> io::Result<Function> {
        let cname = CString::new(name).map_err(|e| io::Error::other(e.to_string()))?;
        let mut func: CUfunction = std::ptr::null_mut();
        unsafe {
            check(
                self.table,
                (self.table.cu_module_get_function)(&mut func, self.module, cname.as_ptr()),
                "cuModuleGetFunction",
            )?
        };
        Ok(Function {
            func,
            table: self.table,
        })
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        if !self.module.is_null() {
            unsafe { (self.table.cu_module_unload)(self.module) };
        }
    }
}

/// Handle to a kernel function. Cheap to clone (just a pointer).
#[derive(Clone)]
pub struct Function {
    pub(crate) func: CUfunction,
    #[allow(dead_code)] // holds CudaTable alive for RAII; not actively read
    pub(crate) table: &'static CudaTable,
}

/// Owned device memory allocation.
pub struct DeviceBuffer {
    pub(crate) ptr: CUdeviceptr,
    pub(crate) bytes: usize,
    pub(crate) table: &'static CudaTable,
}

impl DeviceBuffer {
    pub fn alloc(bytes: usize) -> io::Result<Self> {
        let table = ensure_cuda_init()?;
        let mut ptr: CUdeviceptr = 0;
        unsafe { check(table, (table.cu_mem_alloc_v2)(&mut ptr, bytes), "cuMemAlloc_v2")? };
        Ok(Self { ptr, bytes, table })
    }

    pub fn copy_from_host(&self, src: &[u8]) -> io::Result<()> {
        if src.len() > self.bytes {
            return Err(io::Error::other("DeviceBuffer::copy_from_host overflow"));
        }
        unsafe {
            check(
                self.table,
                (self.table.cu_memcpy_h_to_d_v2)(
                    self.ptr,
                    src.as_ptr() as *const c_void,
                    src.len(),
                ),
                "cuMemcpyHtoD_v2",
            )
        }
    }

    pub fn copy_to_host(&self, dst: &mut [u8]) -> io::Result<()> {
        if dst.len() > self.bytes {
            return Err(io::Error::other("DeviceBuffer::copy_to_host overflow"));
        }
        unsafe {
            check(
                self.table,
                (self.table.cu_memcpy_d_to_h_v2)(
                    dst.as_mut_ptr() as *mut c_void,
                    self.ptr,
                    dst.len(),
                ),
                "cuMemcpyDtoH_v2",
            )
        }
    }

    pub fn ptr(&self) -> CUdeviceptr {
        self.ptr
    }
}

impl Drop for DeviceBuffer {
    fn drop(&mut self) {
        if self.ptr != 0 {
            unsafe { (self.table.cu_mem_free_v2)(self.ptr) };
        }
    }
}

/// Pinned (page-locked) host memory. ~2× faster PCIe transfers than
/// regular Vec<u8> because the DMA engine can transfer without going
/// through the OS page tables.
pub struct PinnedBuffer<T> {
    ptr: *mut T,
    len: usize,
    table: &'static CudaTable,
}

impl<T: Copy> PinnedBuffer<T> {
    pub fn new(len: usize) -> io::Result<Self> {
        let table = ensure_cuda_init()?;
        let bytes = len * std::mem::size_of::<T>();
        let mut p: *mut c_void = std::ptr::null_mut();
        unsafe {
            check(
                table,
                (table.cu_mem_alloc_host_v2)(&mut p, bytes),
                "cuMemAllocHost_v2",
            )?
        };
        Ok(Self {
            ptr: p as *mut T,
            len,
            table,
        })
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T> Drop for PinnedBuffer<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { (self.table.cu_mem_free_host)(self.ptr as *mut c_void) };
        }
    }
}

// SAFETY: PinnedBuffer is just a chunk of host memory. Send+Sync as long
// as T is — same guarantees as Vec<T>.
unsafe impl<T: Send> Send for PinnedBuffer<T> {}
unsafe impl<T: Sync> Sync for PinnedBuffer<T> {}

/// Async stream. Submit copies and launches; they execute in order on
/// this stream but in parallel with other streams.
pub struct Stream {
    pub(crate) stream: CUstream,
    pub(crate) table: &'static CudaTable,
}

impl Stream {
    pub fn new() -> io::Result<Self> {
        let table = ensure_cuda_init()?;
        let mut stream: CUstream = std::ptr::null_mut();
        unsafe {
            check(
                table,
                (table.cu_stream_create)(&mut stream, 0),
                "cuStreamCreate",
            )?
        };
        Ok(Self { stream, table })
    }

    pub fn synchronize(&self) -> io::Result<()> {
        unsafe {
            check(
                self.table,
                (self.table.cu_stream_synchronize)(self.stream),
                "cuStreamSynchronize",
            )
        }
    }

    pub fn copy_h_to_d_async<T>(
        &self,
        dst: &DeviceBuffer,
        src: &[T],
    ) -> io::Result<()> {
        let bytes = std::mem::size_of_val(src);
        if bytes > dst.bytes {
            return Err(io::Error::other("copy_h_to_d_async overflow"));
        }
        unsafe {
            check(
                self.table,
                (self.table.cu_memcpy_h_to_d_async_v2)(
                    dst.ptr,
                    src.as_ptr() as *const c_void,
                    bytes,
                    self.stream,
                ),
                "cuMemcpyHtoDAsync_v2",
            )
        }
    }

    pub fn copy_d_to_h_async<T>(
        &self,
        dst: &mut [T],
        src: &DeviceBuffer,
    ) -> io::Result<()> {
        let bytes = std::mem::size_of_val(dst);
        if bytes > src.bytes {
            return Err(io::Error::other("copy_d_to_h_async overflow"));
        }
        unsafe {
            check(
                self.table,
                (self.table.cu_memcpy_d_to_h_async_v2)(
                    dst.as_mut_ptr() as *mut c_void,
                    src.ptr,
                    bytes,
                    self.stream,
                ),
                "cuMemcpyDtoHAsync_v2",
            )
        }
    }

    /// Launches a kernel on this stream. `kernel_params` must be an array
    /// of pointers to each kernel argument's storage — see CUDA Driver
    /// API docs for `cuLaunchKernel` (the contract is unchanged since
    /// CUDA 4.0).
    pub fn launch(
        &self,
        func: &Function,
        grid: (u32, u32, u32),
        block: (u32, u32, u32),
        shared_mem_bytes: u32,
        kernel_params: &mut [*mut c_void],
    ) -> io::Result<()> {
        unsafe {
            check(
                self.table,
                (self.table.cu_launch_kernel)(
                    func.func,
                    grid.0,
                    grid.1,
                    grid.2,
                    block.0,
                    block.1,
                    block.2,
                    shared_mem_bytes,
                    self.stream,
                    kernel_params.as_mut_ptr(),
                    std::ptr::null_mut(),
                ),
                "cuLaunchKernel",
            )
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if !self.stream.is_null() {
            unsafe { (self.table.cu_stream_destroy_v2)(self.stream) };
        }
    }
}

// SAFETY: CUDA driver objects are thread-safe (cuda contexts and streams
// can be used from multiple threads with proper synchronization).
unsafe impl Send for Context {}
unsafe impl Sync for Context {}
unsafe impl Send for Module {}
unsafe impl Sync for Module {}
unsafe impl Send for Function {}
unsafe impl Sync for Function {}
unsafe impl Send for DeviceBuffer {}
unsafe impl Sync for DeviceBuffer {}
unsafe impl Send for Stream {}
unsafe impl Sync for Stream {}

// ─── High-level Forge KASM-on-GPU runtime ───────────────────────────

/// One-time initialization of the KASM interpreter on GPU 0. Returns a
/// shared singleton — the PTX is loaded once, the kernel function is
/// resolved once, the context is reused across all calls.
pub fn kasm_runtime() -> io::Result<&'static KasmGpuRuntime> {
    static RUNTIME: OnceLock<Result<KasmGpuRuntime, String>> = OnceLock::new();
    let entry = RUNTIME.get_or_init(|| {
        KasmGpuRuntime::new(0).map_err(|e| e.to_string())
    });
    entry.as_ref().map_err(|e| io::Error::other(e.clone()))
}

pub struct KasmGpuRuntime {
    pub context: Context,
    pub module: Module,
    pub kernel: Function,
    pub stream: Stream,
}

impl KasmGpuRuntime {
    pub fn new(device_ordinal: i32) -> io::Result<Self> {
        let context = Context::new(device_ordinal)?;
        context.set_current()?;
        let module = Module::load_image(KASM_INTERPRET_FATBIN)?;
        let kernel = module.get_function("kasm_interpret")?;
        let stream = Stream::new()?;
        Ok(Self {
            context,
            module,
            kernel,
            stream,
        })
    }

    /// Run the universal KASM interpreter on a flat batch of i64 inputs.
    /// Returns one i64 output per input. Uses pinned memory and async
    /// copy/launch/copy pipeline for maximum throughput.
    pub fn run_batch(
        &self,
        program_bytes: &[u8],
        n_nodes: u32,
        inputs: &[i64],
    ) -> io::Result<Vec<i64>> {
        self.context.set_current()?;
        let n = inputs.len();
        if n == 0 {
            return Ok(Vec::new());
        }

        // Pin host buffers for fast PCIe transfers.
        let mut h_inputs: PinnedBuffer<i64> = PinnedBuffer::new(n)?;
        h_inputs.as_mut_slice().copy_from_slice(inputs);
        let mut h_outputs: PinnedBuffer<i64> = PinnedBuffer::new(n)?;

        // Allocate device buffers.
        let d_program = DeviceBuffer::alloc(program_bytes.len())?;
        let d_inputs = DeviceBuffer::alloc(n * std::mem::size_of::<i64>())?;
        let d_outputs = DeviceBuffer::alloc(n * std::mem::size_of::<i64>())?;

        // Async pipeline: program copy → inputs copy → kernel → outputs copy.
        self.stream
            .copy_h_to_d_async(&d_program, program_bytes)?;
        self.stream
            .copy_h_to_d_async(&d_inputs, h_inputs.as_slice())?;

        // Launch kernel: block size 256, grid sized to cover n threads.
        let block_size: u32 = 256;
        let grid_size: u32 = ((n as u64 + block_size as u64 - 1) / block_size as u64) as u32;

        let mut prog_ptr = d_program.ptr();
        let mut inputs_ptr = d_inputs.ptr();
        let mut outputs_ptr = d_outputs.ptr();
        let mut n_nodes_arg = n_nodes;
        let mut n_threads_arg = n as u32;

        let mut params = [
            &mut prog_ptr as *mut _ as *mut c_void,
            &mut n_nodes_arg as *mut _ as *mut c_void,
            &mut inputs_ptr as *mut _ as *mut c_void,
            &mut outputs_ptr as *mut _ as *mut c_void,
            &mut n_threads_arg as *mut _ as *mut c_void,
        ];

        self.stream.launch(
            &self.kernel,
            (grid_size, 1, 1),
            (block_size, 1, 1),
            0,
            &mut params,
        )?;

        self.stream
            .copy_d_to_h_async(h_outputs.as_mut_slice(), &d_outputs)?;

        // Single sync at the end — overlap all preceding work.
        self.stream.synchronize()?;

        Ok(h_outputs.as_slice().to_vec())
    }
}

pub fn synth_runtime() -> io::Result<&'static SynthGpuRuntime> {
    static RUNTIME: OnceLock<Result<SynthGpuRuntime, String>> = OnceLock::new();
    let entry = RUNTIME.get_or_init(|| {
        SynthGpuRuntime::new(0).map_err(|e| e.to_string())
    });
    entry.as_ref().map_err(|e| io::Error::other(e.clone()))
}

pub struct SynthGpuRuntime {
    pub context: Context,
    pub module: Module,
    pub job_kernel: Function,
    pub pair_kernel: Function,
    pub pair_vec2_kernel: Function,
    pub stream: Stream,
}

impl SynthGpuRuntime {
    pub fn new(device: i32) -> io::Result<Self> {
        let context = Context::new(device)?;
        context.set_current()?;
        let module = Module::load_image(KASM_INTERPRET_FATBIN)?;
        let job_kernel = module.get_function("synth_score")?;
        let pair_kernel = module.get_function("synth_score_pairs")?;
        let pair_vec2_kernel = module.get_function("synth_score_pairs_vec2")?;
        let stream = Stream::new()?;
        Ok(Self {
            context,
            module,
            job_kernel,
            pair_kernel,
            pair_vec2_kernel,
            stream,
        })
    }
}

/// Best-effort probe — returns true if the CUDA driver is loadable AND
/// at least one device exists AND the embedded KASM fatbin is non-empty.
/// Used by the GPU bootstrap to decide whether the cuda_min path is
/// even an option on this machine, before trying anything heavy.
pub fn cuda_min_available() -> bool {
    if KASM_INTERPRET_FATBIN.is_empty() {
        return false;
    }
    let table = match ensure_cuda_init() {
        Ok(t) => t,
        Err(_) => return false,
    };
    let mut count: i32 = 0;
    unsafe { (table.cu_device_get_count)(&mut count) };
    count > 0
}

/// Run the synth scoring kernel on CUDA.
/// Each job = (pair_idx, op_idx) scores one combination across all examples.
/// Returns raw bytes: per job = (loss_lo: u64, loss_hi: u64, fingerprint: u64).
pub fn synth_score_batch(
    candidates: &[i64],
    targets: &[i64],
    pairs: &[u32],
    jobs: &[u32],
    n_examples: u32,
    n_pairs: u32,
) -> io::Result<Vec<u8>> {
    let total_jobs = jobs.len();
    let out_bytes = total_jobs * 24; // 3 × u64 per job

    let runtime = synth_runtime()?;
    runtime.context.set_current()?;

    let d_candidates = DeviceBuffer::alloc(candidates.len() * 8)?;
    let d_targets = DeviceBuffer::alloc(targets.len() * 8)?;
    let d_pairs = DeviceBuffer::alloc(pairs.len() * 4)?;
    let d_jobs = DeviceBuffer::alloc(jobs.len() * 4)?;
    let d_results = DeviceBuffer::alloc(out_bytes)?;

    let cand_bytes = unsafe {
        std::slice::from_raw_parts(candidates.as_ptr() as *const u8, candidates.len() * 8)
    };
    let tgt_bytes = unsafe {
        std::slice::from_raw_parts(targets.as_ptr() as *const u8, targets.len() * 8)
    };
    let pair_bytes = unsafe {
        std::slice::from_raw_parts(pairs.as_ptr() as *const u8, pairs.len() * 4)
    };
    let job_bytes = unsafe {
        std::slice::from_raw_parts(jobs.as_ptr() as *const u8, jobs.len() * 4)
    };

    d_candidates.copy_from_host(cand_bytes)?;
    d_targets.copy_from_host(tgt_bytes)?;
    d_pairs.copy_from_host(pair_bytes)?;
    d_jobs.copy_from_host(job_bytes)?;

    let mut p_cand = d_candidates.ptr();
    let mut p_tgt = d_targets.ptr();
    let mut p_pairs = d_pairs.ptr();
    let mut p_jobs = d_jobs.ptr();
    let mut p_results = d_results.ptr();
    let mut p_n_examples = n_examples;
    let mut p_n_pairs = n_pairs;
    let mut p_n_jobs = total_jobs as u32;

    let mut params: [*mut c_void; 8] = [
        &mut p_cand as *mut _ as *mut c_void,
        &mut p_tgt as *mut _ as *mut c_void,
        &mut p_pairs as *mut _ as *mut c_void,
        &mut p_jobs as *mut _ as *mut c_void,
        &mut p_results as *mut _ as *mut c_void,
        &mut p_n_examples as *mut _ as *mut c_void,
        &mut p_n_pairs as *mut _ as *mut c_void,
        &mut p_n_jobs as *mut _ as *mut c_void,
    ];

    let block_size = 256u32;
    let grid_size = ((total_jobs as u32) + block_size - 1) / block_size;

    runtime.stream.launch(
        &runtime.job_kernel,
        (grid_size, 1, 1),
        (block_size, 1, 1),
        0,
        &mut params,
    )?;

    let mut output = vec![0u8; out_bytes];
    d_results.copy_to_host(&mut output)?;
    runtime.stream.synchronize()?;

    Ok(output)
}

/// Run the dense pair-fused synth scoring kernel on CUDA.
/// One CUDA thread scores all 9 opcodes for one pair, preserving the
/// standard dense output order: pair0 op0..8, pair1 op0..8, ...
pub fn synth_score_dense_pairs(
    candidates: &[i64],
    targets: &[i64],
    pairs: &[u32],
    n_examples: u32,
    n_pairs: u32,
) -> io::Result<Vec<u8>> {
    let total_jobs = pairs.len() * 9;
    let out_bytes = total_jobs * 24; // 3 x u64 per job

    let runtime = synth_runtime()?;
    runtime.context.set_current()?;

    let d_candidates = DeviceBuffer::alloc(candidates.len() * 8)?;
    let d_targets = DeviceBuffer::alloc(targets.len() * 8)?;
    let d_pairs = DeviceBuffer::alloc(pairs.len() * 4)?;
    let d_results = DeviceBuffer::alloc(out_bytes)?;

    let cand_bytes = unsafe {
        std::slice::from_raw_parts(candidates.as_ptr() as *const u8, candidates.len() * 8)
    };
    let tgt_bytes = unsafe {
        std::slice::from_raw_parts(targets.as_ptr() as *const u8, targets.len() * 8)
    };
    let pair_bytes = unsafe {
        std::slice::from_raw_parts(pairs.as_ptr() as *const u8, pairs.len() * 4)
    };

    d_candidates.copy_from_host(cand_bytes)?;
    d_targets.copy_from_host(tgt_bytes)?;
    d_pairs.copy_from_host(pair_bytes)?;

    let mut p_cand = d_candidates.ptr();
    let mut p_tgt = d_targets.ptr();
    let mut p_pairs = d_pairs.ptr();
    let mut p_results = d_results.ptr();
    let mut p_n_examples = n_examples;
    let mut p_n_pairs = n_pairs;
    let mut p_n_ops = 9u32;

    let mut params: [*mut c_void; 7] = [
        &mut p_cand as *mut _ as *mut c_void,
        &mut p_tgt as *mut _ as *mut c_void,
        &mut p_pairs as *mut _ as *mut c_void,
        &mut p_results as *mut _ as *mut c_void,
        &mut p_n_examples as *mut _ as *mut c_void,
        &mut p_n_pairs as *mut _ as *mut c_void,
        &mut p_n_ops as *mut _ as *mut c_void,
    ];

    let block_size = 256u32;
    let grid_size = (n_pairs + block_size - 1) / block_size;

    runtime.stream.launch(
        &runtime.pair_kernel,
        (grid_size, 1, 1),
        (block_size, 1, 1),
        0,
        &mut params,
    )?;

    let mut output = vec![0u8; out_bytes];
    d_results.copy_to_host(&mut output)?;
    runtime.stream.synchronize()?;

    Ok(output)
}

/// Same dense pair-fused route, but using 128-bit `longlong2` loads.
/// Call only when n_examples is even so every candidate row remains
/// 16-byte aligned.
pub fn synth_score_dense_pairs_vec2(
    candidates: &[i64],
    targets: &[i64],
    pairs: &[u32],
    n_examples: u32,
    n_pairs: u32,
) -> io::Result<Vec<u8>> {
    let total_jobs = pairs.len() * 9;
    let out_bytes = total_jobs * 24; // 3 x u64 per job

    let runtime = synth_runtime()?;
    runtime.context.set_current()?;

    let d_candidates = DeviceBuffer::alloc(candidates.len() * 8)?;
    let d_targets = DeviceBuffer::alloc(targets.len() * 8)?;
    let d_pairs = DeviceBuffer::alloc(pairs.len() * 4)?;
    let d_results = DeviceBuffer::alloc(out_bytes)?;

    let cand_bytes = unsafe {
        std::slice::from_raw_parts(candidates.as_ptr() as *const u8, candidates.len() * 8)
    };
    let tgt_bytes = unsafe {
        std::slice::from_raw_parts(targets.as_ptr() as *const u8, targets.len() * 8)
    };
    let pair_bytes = unsafe {
        std::slice::from_raw_parts(pairs.as_ptr() as *const u8, pairs.len() * 4)
    };

    d_candidates.copy_from_host(cand_bytes)?;
    d_targets.copy_from_host(tgt_bytes)?;
    d_pairs.copy_from_host(pair_bytes)?;

    let mut p_cand = d_candidates.ptr();
    let mut p_tgt = d_targets.ptr();
    let mut p_pairs = d_pairs.ptr();
    let mut p_results = d_results.ptr();
    let mut p_n_examples = n_examples;
    let mut p_n_pairs = n_pairs;
    let mut p_n_ops = 9u32;

    let mut params: [*mut c_void; 7] = [
        &mut p_cand as *mut _ as *mut c_void,
        &mut p_tgt as *mut _ as *mut c_void,
        &mut p_pairs as *mut _ as *mut c_void,
        &mut p_results as *mut _ as *mut c_void,
        &mut p_n_examples as *mut _ as *mut c_void,
        &mut p_n_pairs as *mut _ as *mut c_void,
        &mut p_n_ops as *mut _ as *mut c_void,
    ];

    let block_size = 256u32;
    let grid_size = (n_pairs + block_size - 1) / block_size;

    runtime.stream.launch(
        &runtime.pair_vec2_kernel,
        (grid_size, 1, 1),
        (block_size, 1, 1),
        0,
        &mut params,
    )?;

    let mut output = vec![0u8; out_bytes];
    d_results.copy_to_host(&mut output)?;
    runtime.stream.synchronize()?;

    Ok(output)
}
