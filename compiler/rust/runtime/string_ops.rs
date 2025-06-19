// Runtime string operations for Palladium
// These will be available as built-in functions

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[repr(C)]
pub struct PdString {
    data: *mut c_char,
    len: i64,
    capacity: i64,
}

/// String concatenation
///
/// # Safety
/// The caller must ensure that:
/// - Both `a` and `b` are valid pointers to PdString structs
/// - The data field in both structs points to valid null-terminated C strings
/// - The lifetime of the input strings extends through this function call
#[no_mangle]
pub unsafe extern "C" fn pd_string_concat(a: *const PdString, b: *const PdString) -> PdString {
    unsafe {
        let a_str = CStr::from_ptr((*a).data).to_string_lossy();
        let b_str = CStr::from_ptr((*b).data).to_string_lossy();
        let result = format!("{}{}", a_str, b_str);

        let c_string = CString::new(result).unwrap();
        let len = c_string.as_bytes().len() as i64;
        let data = c_string.into_raw();

        PdString {
            data,
            len,
            capacity: len,
        }
    }
}

/// String append (modifies first string)
///
/// # Safety
/// The caller must ensure that:
/// - `a` is a valid mutable pointer to a PdString struct
/// - `b` is a valid pointer to a PdString struct
/// - The data fields in both structs point to valid null-terminated C strings
/// - The caller is responsible for the memory management of the PdString at `a`
#[no_mangle]
pub unsafe extern "C" fn pd_string_append(a: *mut PdString, b: *const PdString) {
    unsafe {
        let a_str = CStr::from_ptr((*a).data).to_string_lossy().into_owned();
        let b_str = CStr::from_ptr((*b).data).to_string_lossy();
        let result = format!("{}{}", a_str, b_str);

        // Free old data
        let _ = CString::from_raw((*a).data);

        let c_string = CString::new(result).unwrap();
        let len = c_string.as_bytes().len() as i64;
        let data = c_string.into_raw();

        (*a).data = data;
        (*a).len = len;
        (*a).capacity = len;
    }
}

// Create string from integer
#[no_mangle]
pub extern "C" fn pd_int_to_string(n: i64) -> PdString {
    let s = n.to_string();
    let c_string = CString::new(s).unwrap();
    let len = c_string.as_bytes().len() as i64;
    let data = c_string.into_raw();

    PdString {
        data,
        len,
        capacity: len,
    }
}

/// String length (already exists but let's make it consistent)
///
/// # Safety
/// The caller must ensure that:
/// - `s` is a valid pointer to a PdString struct
/// - The PdString has been properly initialized
#[no_mangle]
pub unsafe extern "C" fn pd_string_length(s: *const PdString) -> i64 {
    unsafe { (*s).len }
}
