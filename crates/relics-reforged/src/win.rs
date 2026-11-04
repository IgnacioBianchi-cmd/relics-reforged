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

extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry32) -> i32;
    fn GetCurrentProcessId() -> u32;
    fn OpenThread(access: u32, inherit: i32, tid: u32) -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    pub fn SuspendThread(h: Handle) -> u32;
    pub fn ResumeThread(h: Handle) -> u32;
    fn GetThreadContext(h: Handle, ctx: *mut Context) -> i32;
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationThread(
        h: Handle,
        class: u32,
        info: *mut c_void,
        len: u32,
        ret: *mut u32,
    ) -> i32;
}

#[repr(C)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_pid: u32,
    base_pri: i32,
    delta_pri: i32,
    flags: u32,
}

/// CONTEXT de x64: 0x4D0 bytes alineados a 16. Solo usamos unos pocos campos por offset.
#[repr(C, align(16))]
struct Context([u8; 0x4D0]);

/// IDs de los hilos de este proceso.
pub fn process_threads() -> Vec<u32> {
    const TH32CS_SNAPTHREAD: u32 = 0x4;
    let mut out = Vec::new();
    // SAFETY: llamadas de Toolhelp con una estructura nuestra del tamaño declarado.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snap as isize == -1 {
            return out;
        }
        let pid = GetCurrentProcessId();
        let mut e = ThreadEntry32 {
            size: std::mem::size_of::<ThreadEntry32>() as u32,
            usage: 0,
            thread_id: 0,
            owner_pid: 0,
            base_pri: 0,
            delta_pri: 0,
            flags: 0,
        };
        let mut ok = Thread32First(snap, &mut e);
        while ok != 0 {
            if e.owner_pid == pid {
                out.push(e.thread_id);
            }
            ok = Thread32Next(snap, &mut e);
        }
        CloseHandle(snap);
    }
    out
}

pub fn open_thread(tid: u32) -> Option<Handle> {
    const ACCESS: u32 = 0x0002 | 0x0008 | 0x0040; // SUSPEND_RESUME | GET_CONTEXT | QUERY_INFORMATION
                                                  // SAFETY: OpenThread no tiene precondiciones; devuelve NULL si falla.
    let h = unsafe { OpenThread(ACCESS, 0, tid) };
    (!h.is_null()).then_some(h)
}

pub fn close_handle(h: Handle) {
    // SAFETY: cerramos un handle que abrimos nosotros.
    unsafe { CloseHandle(h) };
}

/// Dirección donde arrancó el hilo (ThreadQuerySetWin32StartAddress).
pub fn thread_start_address(h: Handle) -> Option<usize> {
    let mut addr = 0usize;
    // SAFETY: el buffer es un usize nuestro y le pasamos su tamaño.
    let status = unsafe {
        NtQueryInformationThread(
            h,
            9,
            (&mut addr as *mut usize).cast(),
            8,
            std::ptr::null_mut(),
        )
    };
    (status >= 0).then_some(addr)
}

/// RIP y RSP de un hilo suspendido. No pide memoria del heap.
pub fn thread_rip_rsp(h: Handle) -> Option<(usize, usize)> {
    const CONTEXT_CONTROL: u32 = 0x0010_0001;
    let mut ctx = Context([0; 0x4D0]);
    ctx.0[0x30..0x34].copy_from_slice(&CONTEXT_CONTROL.to_le_bytes());
    // SAFETY: CONTEXT alineado y del tamaño correcto, con ContextFlags inicializado.
    if unsafe { GetThreadContext(h, &mut ctx) } == 0 {
        return None;
    }
    let field = |off: usize| usize::from_le_bytes(ctx.0[off..off + 8].try_into().unwrap());
    Some((field(0xF8), field(0x98)))
}

extern "system" {
    fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
    fn FlushInstructionCache(process: Handle, addr: *const c_void, size: usize) -> i32;
}

/// Reserva memoria ejecutable a menos de 2 GB de `near` (para poder llegar con un `call rel32`).
pub fn alloc_executable_near(near: usize, size: usize) -> Option<usize> {
    const MEM_COMMIT_RESERVE: u32 = 0x3000;
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    const STEP: usize = 0x1_0000;
    for i in 1..0x7000usize {
        for candidate in [near.checked_sub(i * STEP), near.checked_add(i * STEP)]
            .into_iter()
            .flatten()
        {
            // SAFETY: pedimos una dirección concreta; si está ocupada devuelve NULL.
            let p = unsafe {
                VirtualAlloc(
                    candidate as *mut c_void,
                    size,
                    MEM_COMMIT_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                )
            };
            if !p.is_null() {
                return Some(p as usize);
            }
        }
    }
    None
}

pub fn flush_instruction_cache(addr: usize, size: usize) {
    // SAFETY: solo invalida la caché de instrucciones de un rango de nuestro proceso.
    unsafe { FlushInstructionCache(GetCurrentProcess(), addr as *const c_void, size) };
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

/// Regiones de memoria privada, comprometida y escribible del proceso: (base, tamaño).
pub fn writable_regions() -> Vec<(usize, usize)> {
    const MEM_COMMIT: u32 = 0x1000;
    const MEM_PRIVATE: u32 = 0x2_0000;
    let mut out = Vec::new();
    let mut addr = 0x1_0000usize;
    while addr < 0x7FFF_FFFE_0000 {
        let mut info = MemoryBasicInformation::default();
        // SAFETY: VirtualQuery solo escribe en nuestra estructura.
        let n = unsafe {
            VirtualQuery(
                addr as *const c_void,
                &mut info,
                std::mem::size_of::<MemoryBasicInformation>(),
            )
        };
        if n == 0 || info.region_size == 0 {
            break;
        }
        let writable = matches!(info.protect & 0xFF, 0x04 | 0x40);
        if info.state == MEM_COMMIT
            && info.kind == MEM_PRIVATE
            && writable
            && info.protect & PAGE_GUARD == 0
        {
            out.push((info.base, info.region_size));
        }
        addr = info.base + info.region_size;
    }
    out
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

/// Llena `buf` con lo que hay desde `addr`, página por página (las ilegibles quedan en cero).
pub fn read_into(addr: usize, buf: &mut [u8]) {
    let mut pos = 0;
    while pos < buf.len() {
        let chunk = (0x1000 - (addr + pos) % 0x1000).min(buf.len() - pos);
        let mut read = 0usize;
        // SAFETY: ReadProcessMemory valida el origen; el destino es un tramo de nuestro buffer.
        let ok = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                (addr + pos) as *const c_void,
                buf[pos..].as_mut_ptr().cast(),
                chunk,
                &mut read,
            )
        };
        if ok == 0 || read != chunk {
            buf[pos..pos + chunk].fill(0);
        }
        pos += chunk;
    }
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
