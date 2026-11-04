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
    fn WriteProcessMemory(
        process: Handle,
        base: *mut c_void,
        buffer: *const c_void,
        size: usize,
        written: *mut usize,
    ) -> i32;
    fn GetModuleFileNameW(module: Handle, buffer: *mut u16, size: u32) -> u32;
    pub fn DisableThreadLibraryCalls(module: Handle) -> i32;
    pub fn VirtualProtect(addr: *mut c_void, size: usize, new: u32, old: *mut u32) -> i32;
    fn VirtualQuery(addr: *const c_void, info: *mut MemoryBasicInformation, len: usize) -> usize;
    pub fn AddVectoredExceptionHandler(
        first: u32,
        handler: unsafe extern "system" fn(*mut ExceptionPointers) -> i32,
    ) -> Handle;
    pub fn GetCurrentThreadId() -> u32;
}

pub const PAGE_GUARD: u32 = 0x100;

#[repr(C)]
pub struct ExceptionPointers {
    pub record: *mut u8,
    pub context: *mut u8,
}

#[repr(C)]
#[derive(Default)]
struct MemoryBasicInformation {
    base: usize,
    allocation_base: usize,
    allocation_protect: u32,
    partition_id: u16,
    region_size: usize,
    state: u32,
    protect: u32,
    kind: u32,
}

/// Protección actual de la página que contiene `addr` (`None` si no está reservada).
pub fn page_protection(addr: usize) -> Option<u32> {
    let mut info = MemoryBasicInformation::default();
    // SAFETY: VirtualQuery solo escribe en nuestra estructura, del tamaño que le pasamos.
    let n = unsafe {
        VirtualQuery(
            addr as *const c_void,
            &mut info,
            std::mem::size_of::<MemoryBasicInformation>(),
        )
    };
    const MEM_COMMIT: u32 = 0x1000;
    (n != 0 && info.state == MEM_COMMIT).then_some(info.protect)
}

/// Escribe un valor en la memoria del propio proceso; devuelve `false` si la dirección no es escribible.
pub fn write<T: Copy>(addr: usize, value: T) -> bool {
    if addr == 0 {
        return false;
    }
    let mut written = 0usize;
    // SAFETY: WriteProcessMemory valida el destino; el origen es un valor nuestro.
    let ok = unsafe {
        WriteProcessMemory(
            GetCurrentProcess(),
            addr as *mut c_void,
            (&value as *const T).cast(),
            std::mem::size_of::<T>(),
            &mut written,
        )
    };
    ok != 0 && written == std::mem::size_of::<T>()
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

/// Copia `len` bytes desde `addr`, página por página; las páginas ilegibles quedan en cero.
/// Devuelve los bytes y cuántas páginas no se pudieron leer.
pub fn read_bytes(addr: usize, len: usize) -> (Vec<u8>, usize) {
    let mut out = vec![0u8; len];
    let mut failed = 0;
    let mut pos = 0;
    while pos < len {
        let chunk = (0x1000 - (addr + pos) % 0x1000).min(len - pos);
        let mut read = 0usize;
        // SAFETY: ReadProcessMemory valida el origen; el destino es un tramo de nuestro buffer.
        let ok = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                (addr + pos) as *const c_void,
                out[pos..].as_mut_ptr().cast(),
                chunk,
                &mut read,
            )
        };
        if ok == 0 || read != chunk {
            failed += 1;
        }
        pos += chunk;
    }
    (out, failed)
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
