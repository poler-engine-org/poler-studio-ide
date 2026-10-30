//! Zero-copy memory mapping без внешних зависимостей (unix + cross-platform fallback).

use std::io;
use std::path::Path;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::os::raw::{c_int, c_void};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(unix)]
extern "C" {
    fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, length: usize) -> c_int;
}

#[cfg(unix)]
const PROT_READ: c_int = 0x1;
#[cfg(unix)]
const MAP_PRIVATE: c_int = 0x02;

/// Отображение файла только для чтения. Освобождается при `Drop`.
#[cfg(unix)]
pub struct Mmap {
    ptr: *mut u8,
    len: usize,
}

#[cfg(not(unix))]
pub struct Mmap {
    data: Vec<u8>,
}

unsafe impl Send for Mmap {}
unsafe impl Sync for Mmap {}

#[cfg(unix)]
impl Mmap {
    pub fn open(path: &Path) -> io::Result<Mmap> {
        let file = File::open(path)?;
        let len = file.metadata()?.len() as usize;
        if len == 0 {
            return Ok(Mmap {
                ptr: std::ptr::NonNull::<u8>::dangling().as_ptr(),
                len: 0,
            });
        }
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                len,
                PROT_READ,
                MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            )
        };
        if ptr as usize == usize::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(Mmap {
            ptr: ptr as *mut u8,
            len,
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(unix)]
impl Drop for Mmap {
    fn drop(&mut self) {
        if self.len != 0 {
            unsafe {
                munmap(self.ptr as *mut c_void, self.len);
            }
        }
    }
}

#[cfg(not(unix))]
impl Mmap {
    pub fn open(path: &Path) -> io::Result<Mmap> {
        Ok(Mmap { data: std::fs::read(path)? })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_error() {
        assert!(Mmap::open(Path::new("/nonexistent/pqw/does-not-exist.poler")).is_err());
    }

    #[test]
    fn empty_file_maps_to_empty_slice() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("pqw_mmap_empty.bin");
        std::fs::write(&path, b"").unwrap();
        let m = Mmap::open(&path).unwrap();
        assert!(m.is_empty());
        assert_eq!(m.as_slice(), b"");
        drop(m);
        let _ = std::fs::remove_file(&path);
    }
}