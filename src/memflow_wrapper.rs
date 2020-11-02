use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use memflow::*;
use memflow_win32::error::{Error, Result};
use memflow_win32::*;

pub type CachedConnectorInstance =
    CachedMemoryAccess<'static, ConnectorInstance, TimedCacheValidator>;

pub type CachedTranslate = CachedVirtualTranslate<DirectTranslate, TimedCacheValidator>;

pub type CachedWin32Kernel = memflow_win32::Kernel<CachedConnectorInstance, CachedTranslate>;

pub type CachedWin32Process = memflow_win32::Win32Process<
    VirtualDMA<CachedConnectorInstance, CachedTranslate, Win32VirtualTranslate>,
>;

static mut MEMFLOW_INSTANCE: Option<Arc<Mutex<Memflow>>> = None;

pub unsafe fn lock_memflow<'a>() -> Result<MutexGuard<'a, Memflow>> {
    if MEMFLOW_INSTANCE.is_none() {
        MEMFLOW_INSTANCE = Some(Arc::new(Mutex::new(Memflow::try_init()?)));
    }

    if let Some(memflow) = MEMFLOW_INSTANCE.as_ref() {
        if let Ok(memflow) = memflow.lock() {
            Ok(memflow)
        } else {
            Err(Error::Other("unable to lock memflow"))
        }
    } else {
        Err(Error::Other("memflow is not properly initialized"))
    }
}

pub struct Memflow {
    pub kernel: CachedWin32Kernel,
    pub handles: HashMap<u32, CachedWin32Process>,
}

impl Memflow {
    // TODO: add config file or gui to setup the connection
    pub fn try_init() -> Result<Self> {
        let inventory = unsafe { ConnectorInventory::scan() };
        let connector = unsafe {
            /*inventory.create_connector(
                "daemon",
                &ConnectorArgs::parse("unix:/var/run/memflow.sock,id=win10").unwrap(),
            )*/

            inventory.create_connector("qemu_procfs", &ConnectorArgs::default())
        }?;

        let kernel = Kernel::builder(connector).build_default_caches().build()?;

        Ok(Self {
            kernel,
            handles: HashMap::new(),
        })
    }

    pub fn open_process(&mut self, pid: u32) -> Result<u32> {
        let proc_info = self.kernel.process_info_pid(pid)?;
        let proc = Win32Process::with_kernel(self.kernel.clone(), proc_info);
        self.handles.insert(pid, proc);
        Ok(pid)
    }

    pub fn close_process(&mut self, handle: u32) {
        self.handles.remove(&handle);
    }

    pub fn get_process_mut(&mut self, handle: u32) -> Option<&mut CachedWin32Process> {
        self.handles.get_mut(&handle)
    }

    // TODO:
    // maybe it would be nice to have a way to update
    // the ProcessInfo directly from a Win32Process instead of going through the kernel again.
    // A alive() function on the process would also be nice
    pub fn is_process_alive(&mut self, handle: u32) -> bool {
        // handle = pid
        if let Ok(proc_info) = self.kernel.process_info_pid(handle) {
            proc_info.exit_status == EXIT_STATUS_STILL_ACTIVE
        } else {
            false
        }
    }
}
