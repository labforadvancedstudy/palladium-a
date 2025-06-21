// Runtime support for I/O operations
// "Bridging Palladium to the system"

#![allow(clippy::not_unsafe_ptr_arg_deref)]

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
#[allow(dead_code)]
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
                
                for entry in dir.flatten() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper function to create a test file
    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    // Helper function to convert string to C-compatible pointer
    fn str_to_ptr(s: &str) -> (*const u8, usize) {
        (s.as_ptr(), s.len())
    }

    #[test]
    fn test_file_open_read_mode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = create_test_file(&temp_dir, "test.txt", "Hello, World!");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Read) };
        assert!(!handle.is_null());
        
        unsafe {
            assert_eq!((*handle).mode as u8, FileMode::Read as u8);
            assert_eq!((*handle).path, path_str);
            assert!((*handle).file.is_some());
            
            let result = pd_file_close(handle);
            assert_eq!(result, 0);
        }
    }

    #[test]
    fn test_file_open_write_mode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Write) };
        assert!(!handle.is_null());
        
        unsafe {
            assert_eq!((*handle).mode as u8, FileMode::Write as u8);
            assert!(file_path.exists());
            
            let result = pd_file_close(handle);
            assert_eq!(result, 0);
        }
    }

    #[test]
    fn test_file_open_invalid_utf8() {
        let invalid_utf8 = [0xFF, 0xFE, 0xFD];
        let handle = unsafe { pd_file_open(invalid_utf8.as_ptr(), invalid_utf8.len(), FileMode::Read) };
        assert!(handle.is_null());
    }

    #[test]
    fn test_file_open_nonexistent() {
        let (path_ptr, path_len) = str_to_ptr("/nonexistent/path/file.txt");
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Read) };
        assert!(handle.is_null());
    }

    #[test]
    fn test_file_read() {
        let temp_dir = TempDir::new().unwrap();
        let content = "Test content for reading";
        let file_path = create_test_file(&temp_dir, "read_test.txt", content);
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Read) };
        assert!(!handle.is_null());
        
        let mut buffer = vec![0u8; 100];
        let bytes_read = unsafe { pd_file_read(handle, buffer.as_mut_ptr(), buffer.len()) };
        assert_eq!(bytes_read, content.len() as i64);
        
        let read_content = String::from_utf8_lossy(&buffer[..bytes_read as usize]);
        assert_eq!(read_content, content);
        
        unsafe { pd_file_close(handle); }
    }

    #[test]
    fn test_file_read_null_handle() {
        let mut buffer = vec![0u8; 10];
        let result = unsafe { pd_file_read(std::ptr::null_mut(), buffer.as_mut_ptr(), buffer.len()) };
        assert_eq!(result, -1);
    }

    #[test]
    fn test_file_read_null_buffer() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = create_test_file(&temp_dir, "test.txt", "content");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Read) };
        let result = unsafe { pd_file_read(handle, std::ptr::null_mut(), 10) };
        assert_eq!(result, -1);
        
        unsafe { pd_file_close(handle); }
    }

    #[test]
    fn test_file_write() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("write_test.txt");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Write) };
        assert!(!handle.is_null());
        
        let content = "Written content";
        let bytes_written = unsafe { pd_file_write(handle, content.as_ptr(), content.len()) };
        assert_eq!(bytes_written, content.len() as i64);
        
        unsafe { pd_file_close(handle); }
        
        // Verify content was written
        let written_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(written_content, content);
    }

    #[test]
    fn test_file_write_null_handle() {
        let content = "test";
        let result = unsafe { pd_file_write(std::ptr::null_mut(), content.as_ptr(), content.len()) };
        assert_eq!(result, -1);
    }

    #[test]
    fn test_file_seek() {
        let temp_dir = TempDir::new().unwrap();
        let content = "0123456789";
        let file_path = create_test_file(&temp_dir, "seek_test.txt", content);
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Read) };
        
        // Seek from start
        let pos = unsafe { pd_file_seek(handle, 0, 5) };
        assert_eq!(pos, 5);
        
        // Seek from current
        let pos = unsafe { pd_file_seek(handle, 1, 2) };
        assert_eq!(pos, 7);
        
        // Seek from end
        let pos = unsafe { pd_file_seek(handle, 2, -3) };
        assert_eq!(pos, 7); // 10 - 3 = 7
        
        // Invalid whence
        let pos = unsafe { pd_file_seek(handle, 3, 0) };
        assert_eq!(pos, -1);
        
        unsafe { pd_file_close(handle); }
    }

    #[test]
    fn test_file_flush() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("flush_test.txt");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Write) };
        
        let content = "Flush test";
        unsafe { pd_file_write(handle, content.as_ptr(), content.len()); }
        
        let result = unsafe { pd_file_flush(handle) };
        assert_eq!(result, 0);
        
        unsafe { pd_file_close(handle); }
    }

    #[test]
    fn test_file_flush_null_handle() {
        let result = unsafe { pd_file_flush(std::ptr::null_mut()) };
        assert_eq!(result, -1);
    }

    #[test]
    fn test_path_exists() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = create_test_file(&temp_dir, "exists.txt", "");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let result = unsafe { pd_path_exists(path_ptr, path_len) };
        assert_eq!(result, 1);
        
        let (path_ptr, path_len) = str_to_ptr("/nonexistent/file.txt");
        let result = unsafe { pd_path_exists(path_ptr, path_len) };
        assert_eq!(result, 0);
    }

    #[test]
    fn test_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = create_test_file(&temp_dir, "file.txt", "");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let result = unsafe { pd_path_is_file(path_ptr, path_len) };
        assert_eq!(result, 1);
        
        let dir_path = temp_dir.path().to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(dir_path);
        let result = unsafe { pd_path_is_file(path_ptr, path_len) };
        assert_eq!(result, 0);
    }

    #[test]
    fn test_path_is_dir() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(dir_path);
        
        let result = unsafe { pd_path_is_dir(path_ptr, path_len) };
        assert_eq!(result, 1);
        
        let file_path = create_test_file(&temp_dir, "file.txt", "");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        let result = unsafe { pd_path_is_dir(path_ptr, path_len) };
        assert_eq!(result, 0);
    }

    #[test]
    fn test_create_dir() {
        let temp_dir = TempDir::new().unwrap();
        let new_dir = temp_dir.path().join("new_directory");
        let path_str = new_dir.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let result = unsafe { pd_create_dir(path_ptr, path_len) };
        assert_eq!(result, 0);
        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
        
        // Try to create again (should fail)
        let result = unsafe { pd_create_dir(path_ptr, path_len) };
        assert_eq!(result, -1);
    }

    #[test]
    fn test_create_dir_all() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("a/b/c/d");
        let path_str = nested_dir.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let result = unsafe { pd_create_dir_all(path_ptr, path_len) };
        assert_eq!(result, 0);
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
    }

    #[test]
    fn test_remove_dir() {
        let temp_dir = TempDir::new().unwrap();
        let dir_to_remove = temp_dir.path().join("removeme");
        fs::create_dir(&dir_to_remove).unwrap();
        
        let path_str = dir_to_remove.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let result = unsafe { pd_remove_dir(path_ptr, path_len) };
        assert_eq!(result, 0);
        assert!(!dir_to_remove.exists());
    }

    #[test]
    fn test_remove_dir_all() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("a/b/c");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("file.txt"), "content").unwrap();
        
        let remove_path = temp_dir.path().join("a");
        let path_str = remove_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let result = unsafe { pd_remove_dir_all(path_ptr, path_len) };
        assert_eq!(result, 0);
        assert!(!remove_path.exists());
    }

    #[test]
    fn test_remove_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = create_test_file(&temp_dir, "remove_me.txt", "content");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let result = unsafe { pd_remove_file(path_ptr, path_len) };
        assert_eq!(result, 0);
        assert!(!file_path.exists());
    }

    #[test]
    fn test_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let content = "Test file content";
        let file_path = create_test_file(&temp_dir, "meta.txt", content);
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let mut metadata = FileMetadata {
            size: 0,
            is_file: 0,
            is_dir: 0,
            is_symlink: 0,
            readonly: 0,
            mode: 0,
            modified_secs: 0,
            accessed_secs: 0,
            created_secs: 0,
        };
        
        let result = unsafe { pd_file_metadata(path_ptr, path_len, &mut metadata) };
        assert_eq!(result, 0);
        assert_eq!(metadata.size, content.len() as u64);
        assert_eq!(metadata.is_file, 1);
        assert_eq!(metadata.is_dir, 0);
        assert!(metadata.modified_secs > 0);
    }

    #[test]
    fn test_file_metadata_null_ptr() {
        let (path_ptr, path_len) = str_to_ptr("/some/path");
        let result = unsafe { pd_file_metadata(path_ptr, path_len, std::ptr::null_mut()) };
        assert_eq!(result, -1);
    }

    #[test]
    fn test_read_dir() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(&temp_dir, "file1.txt", "");
        create_test_file(&temp_dir, "file2.txt", "");
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        
        let path_str = temp_dir.path().to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let mut entries: *mut DirEntry = std::ptr::null_mut();
        let mut count: usize = 0;
        
        let result = unsafe { pd_read_dir(path_ptr, path_len, &mut entries, &mut count) };
        assert_eq!(result, 0);
        assert_eq!(count, 3);
        assert!(!entries.is_null());
        
        // Check entries
        unsafe {
            let entries_slice = std::slice::from_raw_parts(entries, count);
            let mut found_files = 0;
            let mut found_dirs = 0;
            
            for entry in entries_slice {
                if entry.is_file == 1 { found_files += 1; }
                if entry.is_dir == 1 { found_dirs += 1; }
            }
            
            assert_eq!(found_files, 2);
            assert_eq!(found_dirs, 1);
            
            pd_free_dir_entries(entries, count);
        }
    }

    #[test]
    fn test_read_file_to_string() {
        let temp_dir = TempDir::new().unwrap();
        let content = "File content to read";
        let file_path = create_test_file(&temp_dir, "read_string.txt", content);
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let mut out_str: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;
        
        let result = unsafe { pd_read_file_to_string(path_ptr, path_len, &mut out_str, &mut out_len) };
        assert_eq!(result, 0);
        assert_eq!(out_len, content.len());
        
        unsafe {
            let read_content = std::str::from_utf8(std::slice::from_raw_parts(out_str, out_len)).unwrap();
            assert_eq!(read_content, content);
            
            pd_free_string(out_str, out_len);
        }
    }

    #[test]
    fn test_write_string_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("write_string.txt");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let content = "Content to write";
        let result = unsafe { pd_write_string_to_file(path_ptr, path_len, content.as_ptr(), content.len()) };
        assert_eq!(result, 0);
        
        let written_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(written_content, content);
    }

    #[test]
    fn test_io_error_to_code() {
        use std::io::ErrorKind;
        
        let error = io::Error::new(ErrorKind::NotFound, "");
        assert_eq!(io_error_to_code(&error) as u8, IoErrorCode::NotFound as u8);
        
        let error = io::Error::new(ErrorKind::PermissionDenied, "");
        assert_eq!(io_error_to_code(&error) as u8, IoErrorCode::PermissionDenied as u8);
        
        let error = io::Error::new(ErrorKind::AlreadyExists, "");
        assert_eq!(io_error_to_code(&error) as u8, IoErrorCode::AlreadyExists as u8);
        
        let error = io::Error::new(ErrorKind::InvalidInput, "");
        assert_eq!(io_error_to_code(&error) as u8, IoErrorCode::InvalidInput as u8);
        
        let error = io::Error::new(ErrorKind::UnexpectedEof, "");
        assert_eq!(io_error_to_code(&error) as u8, IoErrorCode::UnexpectedEof as u8);
        
        let error = io::Error::new(ErrorKind::Other, "");
        assert_eq!(io_error_to_code(&error) as u8, IoErrorCode::Other as u8);
    }

    #[test]
    fn test_file_close_null_handle() {
        let result = unsafe { pd_file_close(std::ptr::null_mut()) };
        assert_eq!(result, -1);
    }

    #[test]
    fn test_file_append_mode() {
        let temp_dir = TempDir::new().unwrap();
        let initial_content = "Initial content\n";
        let file_path = create_test_file(&temp_dir, "append.txt", initial_content);
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::Append) };
        assert!(!handle.is_null());
        
        let append_content = "Appended content";
        unsafe { pd_file_write(handle, append_content.as_ptr(), append_content.len()); }
        unsafe { pd_file_close(handle); }
        
        let final_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(final_content, format!("{}{}", initial_content, append_content));
    }

    #[test]
    fn test_file_read_write_mode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = create_test_file(&temp_dir, "rw.txt", "Original");
        let path_str = file_path.to_str().unwrap();
        let (path_ptr, path_len) = str_to_ptr(path_str);
        
        let handle = unsafe { pd_file_open(path_ptr, path_len, FileMode::ReadWrite) };
        assert!(!handle.is_null());
        
        // Read original content
        let mut buffer = vec![0u8; 100];
        let bytes_read = unsafe { pd_file_read(handle, buffer.as_mut_ptr(), buffer.len()) };
        assert_eq!(bytes_read, 8);
        
        // Seek to beginning and write new content
        unsafe { pd_file_seek(handle, 0, 0); }
        let new_content = "Modified";
        unsafe { pd_file_write(handle, new_content.as_ptr(), new_content.len()); }
        unsafe { pd_file_close(handle); }
        
        let final_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(final_content, "Modified");
    }

    #[test]
    fn test_free_string_null_ptr() {
        // Should not panic
        unsafe { pd_free_string(std::ptr::null_mut(), 0); }
    }

    #[test]
    fn test_free_dir_entries_null_ptr() {
        // Should not panic
        unsafe { pd_free_dir_entries(std::ptr::null_mut(), 0); }
    }
}