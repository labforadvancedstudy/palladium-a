// Runtime support for I/O operations
// "Bridging Palladium to the system"

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom as StdSeekFrom};
use std::path::Path;
use std::os::unix::fs::PermissionsExt;
use std::time::SystemTime;

/// File handle wrapper
#[repr(C)]
pub struct FileHandle {
    file: Option<File>,
    path: String,
    mode: FileMode,
}

/// File mode
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum FileMode {
    Read = 0,
    Write = 1,
    Append = 2,
    ReadWrite = 3,
}

/// Seek position
#[repr(C)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

/// I/O error codes
#[repr(C)]
#[derive(Debug)]
pub enum IoErrorCode {
    NotFound = 0,
    PermissionDenied = 1,
    AlreadyExists = 2,
    InvalidInput = 3,
    UnexpectedEof = 4,
    Other = 5,
}

/// Convert Rust io::Error to our error code
fn io_error_to_code(err: &io::Error) -> IoErrorCode {
    match err.kind() {
        io::ErrorKind::NotFound => IoErrorCode::NotFound,
        io::ErrorKind::PermissionDenied => IoErrorCode::PermissionDenied,
        io::ErrorKind::AlreadyExists => IoErrorCode::AlreadyExists,
        io::ErrorKind::InvalidInput => IoErrorCode::InvalidInput,
        io::ErrorKind::UnexpectedEof => IoErrorCode::UnexpectedEof,
        _ => IoErrorCode::Other,
    }
}

// File operations - C-compatible exports

#[no_mangle]
pub extern "C" fn pd_file_open(path: *const u8, path_len: usize, mode: FileMode) -> *mut FileHandle {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        let file = match mode {
            FileMode::Read => File::open(path_str),
            FileMode::Write => File::create(path_str),
            FileMode::Append => OpenOptions::new().append(true).open(path_str),
            FileMode::ReadWrite => OpenOptions::new().read(true).write(true).open(path_str),
        };

        match file {
            Ok(f) => {
                let handle = Box::new(FileHandle {
                    file: Some(f),
                    path: path_str.to_string(),
                    mode,
                });
                Box::into_raw(handle)
            }
            Err(_) => std::ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_file_close(handle: *mut FileHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }
    
    unsafe {
        let _ = Box::from_raw(handle);
        0
    }
}

#[no_mangle]
pub extern "C" fn pd_file_read(handle: *mut FileHandle, buffer: *mut u8, len: usize) -> i64 {
    if handle.is_null() || buffer.is_null() {
        return -1;
    }

    unsafe {
        let handle = &mut *handle;
        if let Some(ref mut file) = handle.file {
            let buffer_slice = std::slice::from_raw_parts_mut(buffer, len);
            match file.read(buffer_slice) {
                Ok(n) => n as i64,
                Err(_) => -1,
            }
        } else {
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_file_write(handle: *mut FileHandle, buffer: *const u8, len: usize) -> i64 {
    if handle.is_null() || buffer.is_null() {
        return -1;
    }

    unsafe {
        let handle = &mut *handle;
        if let Some(ref mut file) = handle.file {
            let buffer_slice = std::slice::from_raw_parts(buffer, len);
            match file.write(buffer_slice) {
                Ok(n) => n as i64,
                Err(_) => -1,
            }
        } else {
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_file_seek(handle: *mut FileHandle, whence: u8, offset: i64) -> i64 {
    if handle.is_null() {
        return -1;
    }

    unsafe {
        let handle = &mut *handle;
        if let Some(ref mut file) = handle.file {
            let pos = match whence {
                0 => StdSeekFrom::Start(offset as u64),
                1 => StdSeekFrom::Current(offset),
                2 => StdSeekFrom::End(offset),
                _ => return -1,
            };
            
            match file.seek(pos) {
                Ok(n) => n as i64,
                Err(_) => -1,
            }
        } else {
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_file_flush(handle: *mut FileHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }

    unsafe {
        let handle = &mut *handle;
        if let Some(ref mut file) = handle.file {
            match file.flush() {
                Ok(_) => 0,
                Err(_) => -1,
            }
        } else {
            -1
        }
    }
}

// Path operations

#[no_mangle]
pub extern "C" fn pd_path_exists(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        
        if Path::new(path_str).exists() { 1 } else { 0 }
    }
}

#[no_mangle]
pub extern "C" fn pd_path_is_file(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        
        if Path::new(path_str).is_file() { 1 } else { 0 }
    }
}

#[no_mangle]
pub extern "C" fn pd_path_is_dir(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        
        if Path::new(path_str).is_dir() { 1 } else { 0 }
    }
}

// Directory operations

#[no_mangle]
pub extern "C" fn pd_create_dir(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::create_dir(path_str) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_create_dir_all(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::create_dir_all(path_str) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_remove_dir(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::remove_dir(path_str) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_remove_dir_all(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::remove_dir_all(path_str) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_remove_file(path: *const u8, path_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::remove_file(path_str) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

// File metadata

#[repr(C)]
pub struct FileMetadata {
    size: u64,
    is_file: u8,
    is_dir: u8,
    is_symlink: u8,
    readonly: u8,
    mode: u32,
    modified_secs: i64,
    accessed_secs: i64,
    created_secs: i64,
}

#[no_mangle]
pub extern "C" fn pd_file_metadata(path: *const u8, path_len: usize, metadata: *mut FileMetadata) -> i32 {
    if metadata.is_null() {
        return -1;
    }

    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::metadata(path_str) {
            Ok(meta) => {
                let metadata = &mut *metadata;
                metadata.size = meta.len();
                metadata.is_file = if meta.is_file() { 1 } else { 0 };
                metadata.is_dir = if meta.is_dir() { 1 } else { 0 };
                metadata.is_symlink = if meta.file_type().is_symlink() { 1 } else { 0 };
                metadata.readonly = if meta.permissions().readonly() { 1 } else { 0 };
                metadata.mode = meta.permissions().mode();
                
                // Time handling
                metadata.modified_secs = meta.modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                    
                metadata.accessed_secs = meta.accessed()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                    
                metadata.created_secs = meta.created()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                
                0
            }
            Err(_) => -1,
        }
    }
}

// Directory listing

#[repr(C)]
pub struct DirEntry {
    name: *mut u8,
    name_len: usize,
    is_file: u8,
    is_dir: u8,
}

#[no_mangle]
pub extern "C" fn pd_read_dir(path: *const u8, path_len: usize, entries: *mut *mut DirEntry, count: *mut usize) -> i32 {
    if entries.is_null() || count.is_null() {
        return -1;
    }

    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::read_dir(path_str) {
            Ok(dir) => {
                let mut entry_vec = Vec::new();
                
                for entry in dir {
                    if let Ok(entry) = entry {
                        if let Some(name) = entry.file_name().to_str() {
                            let name_bytes = name.as_bytes();
                            let name_copy = name_bytes.to_vec().into_boxed_slice();
                            let name_ptr = Box::into_raw(name_copy) as *mut u8;
                            
                            let file_type = entry.file_type().ok();
                            let de = DirEntry {
                                name: name_ptr,
                                name_len: name_bytes.len(),
                                is_file: file_type.map(|t| if t.is_file() { 1 } else { 0 }).unwrap_or(0),
                                is_dir: file_type.map(|t| if t.is_dir() { 1 } else { 0 }).unwrap_or(0),
                            };
                            entry_vec.push(de);
                        }
                    }
                }
                
                *count = entry_vec.len();
                let entries_array = entry_vec.into_boxed_slice();
                *entries = Box::into_raw(entries_array) as *mut DirEntry;
                
                0
            }
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_free_dir_entries(entries: *mut DirEntry, count: usize) {
    if entries.is_null() {
        return;
    }
    
    unsafe {
        let entries_slice = std::slice::from_raw_parts_mut(entries, count);
        for entry in &mut *entries_slice {
            if !entry.name.is_null() {
                let _ = Box::from_raw(std::slice::from_raw_parts_mut(entry.name, entry.name_len));
            }
        }
        let _ = Box::from_raw(entries);
    }
}

// Convenience functions

#[no_mangle]
pub extern "C" fn pd_read_file_to_string(path: *const u8, path_len: usize, out_str: *mut *mut u8, out_len: *mut usize) -> i32 {
    if out_str.is_null() || out_len.is_null() {
        return -1;
    }

    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        match std::fs::read_to_string(path_str) {
            Ok(contents) => {
                let bytes = contents.into_bytes().into_boxed_slice();
                *out_len = bytes.len();
                *out_str = Box::into_raw(bytes) as *mut u8;
                0
            }
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_write_string_to_file(path: *const u8, path_len: usize, data: *const u8, data_len: usize) -> i32 {
    unsafe {
        let path_slice = std::slice::from_raw_parts(path, path_len);
        let path_str = match std::str::from_utf8(path_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        let data_slice = std::slice::from_raw_parts(data, data_len);
        
        match std::fs::write(path_str, data_slice) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

#[no_mangle]
pub extern "C" fn pd_free_string(str: *mut u8, len: usize) {
    if str.is_null() {
        return;
    }
    
    unsafe {
        let _ = Box::from_raw(std::slice::from_raw_parts_mut(str, len));
    }
}