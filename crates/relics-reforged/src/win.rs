//! Las pocas funciones de Windows que usa la DLL, declaradas a mano (kernel32 ya está enlazado).

use std::ffi::c_void;

pub type Handle = *mut c_void;

extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetCurrentProcess() -> Handle;
    fn ReadProcessMemory(
        process: Handle,
        base: *const c_void,
        buffer: *mut c_void,
        size: usize,
        read: *mut usize,
    ) -> i32;
    fn GetModuleFileNameW(module: Handle, buffer: *mut u16, size: u32) -> u32;
    pub fn DisableThreadLibraryCalls(module: Handle) -> i32;
}

/// Dirección base del ejecutable del juego (`nightreign.exe`).
pub fn main_module_base() -> usize {
    // SAFETY: con NULL devuelve el módulo del proceso; no falla.
    unsafe { GetModuleHandleW(std::ptr::null()) as usize }
}

/// Lee un valor de la memoria del propio proceso sin riesgo de crash:
/// si la dirección no es legible devuelve `None` en vez de provocar una excepción.
pub fn read<T: Copy>(addr: usize) -> Option<T> {
    if addr == 0 {
        return None;
    }
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut read = 0usize;
    // SAFETY: ReadProcessMemory valida la dirección de origen; el destino es nuestro buffer.
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            addr as *const c_void,
            value.as_mut_ptr().cast(),
            std::mem::size_of::<T>(),
            &mut read,
        )
    };
    // SAFETY: si se leyeron todos los bytes, el valor está inicializado.
    (ok != 0 && read == std::mem::size_of::<T>()).then(|| unsafe { value.assume_init() })
}

/// Ruta completa de un módulo cargado (por ejemplo, esta DLL).
pub fn module_path(module: usize) -> Option<std::path::PathBuf> {
    let mut buf = vec![0u16; 1024];
    // SAFETY: el buffer es nuestro y le pasamos su tamaño real.
    let len = unsafe { GetModuleFileNameW(module as Handle, buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 || len as usize >= buf.len() {
        return None;
    }
    let s = String::from_utf16_lossy(&buf[..len as usize]);
    Some(std::path::PathBuf::from(s))
}

/// Datos de la cabecera PE de un módulo: (TimeDateStamp, SizeOfImage).
pub fn pe_info(base: usize) -> Option<(u32, u32)> {
    let e_lfanew: u32 = read(base + 0x3C)?;
    let nt = base + e_lfanew as usize;
    if read::<u32>(nt)? != 0x0000_4550 {
        return None; // "PE\0\0"
    }
    let timestamp: u32 = read(nt + 0x08)?;
    let size_of_image: u32 = read(nt + 0x18 + 0x38)?;
    Some((timestamp, size_of_image))
}
