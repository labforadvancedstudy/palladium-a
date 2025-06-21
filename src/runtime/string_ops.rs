// Runtime string operations for Palladium
// These will be available as built-in functions

#![allow(clippy::not_unsafe_ptr_arg_deref)]

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    // Helper function to create a PdString from a Rust string
    fn create_pd_string(s: &str) -> PdString {
        let c_string = CString::new(s).unwrap();
        let len = c_string.as_bytes().len() as i64;
        let data = c_string.into_raw();
        PdString {
            data,
            len,
            capacity: len,
        }
    }

    // Helper function to get string content from PdString
    unsafe fn pd_string_to_rust(pd_str: &PdString) -> String {
        if pd_str.data.is_null() {
            String::new()
        } else {
            CStr::from_ptr(pd_str.data).to_string_lossy().into_owned()
        }
    }

    // Helper function to free a PdString
    unsafe fn free_pd_string(pd_str: PdString) {
        if !pd_str.data.is_null() {
            let _ = CString::from_raw(pd_str.data);
        }
    }

    #[test]
    fn test_string_concat() {
        let a = create_pd_string("Hello, ");
        let b = create_pd_string("World!");
        
        let result = unsafe { pd_string_concat(&a, &b) };
        
        unsafe {
            assert_eq!(pd_string_to_rust(&result), "Hello, World!");
            assert_eq!(result.len, 13);
            assert_eq!(result.capacity, 13);
            
            free_pd_string(a);
            free_pd_string(b);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_string_concat_empty_strings() {
        let a = create_pd_string("");
        let b = create_pd_string("Test");
        
        let result1 = unsafe { pd_string_concat(&a, &b) };
        unsafe {
            assert_eq!(pd_string_to_rust(&result1), "Test");
            free_pd_string(result1);
        }
        
        let result2 = unsafe { pd_string_concat(&b, &a) };
        unsafe {
            assert_eq!(pd_string_to_rust(&result2), "Test");
            free_pd_string(result2);
        }
        
        let empty1 = create_pd_string("");
        let empty2 = create_pd_string("");
        let result3 = unsafe { pd_string_concat(&empty1, &empty2) };
        unsafe {
            assert_eq!(pd_string_to_rust(&result3), "");
            free_pd_string(empty1);
            free_pd_string(empty2);
            free_pd_string(result3);
        }
        
        unsafe {
            free_pd_string(a);
            free_pd_string(b);
        }
    }

    #[test]
    fn test_string_concat_special_chars() {
        let a = create_pd_string("Hello\n");
        let b = create_pd_string("\tWorld!");
        
        let result = unsafe { pd_string_concat(&a, &b) };
        
        unsafe {
            assert_eq!(pd_string_to_rust(&result), "Hello\n\tWorld!");
            free_pd_string(a);
            free_pd_string(b);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_string_concat_unicode() {
        let a = create_pd_string("Hello 世界");
        let b = create_pd_string(" 🦀");
        
        let result = unsafe { pd_string_concat(&a, &b) };
        
        unsafe {
            assert_eq!(pd_string_to_rust(&result), "Hello 世界 🦀");
            free_pd_string(a);
            free_pd_string(b);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_string_append() {
        let mut a = create_pd_string("Hello");
        let b = create_pd_string(", World!");
        
        unsafe {
            pd_string_append(&mut a, &b);
            assert_eq!(pd_string_to_rust(&a), "Hello, World!");
            assert_eq!(a.len, 13);
            assert_eq!(a.capacity, 13);
            
            free_pd_string(a);
            free_pd_string(b);
        }
    }

    #[test]
    fn test_string_append_empty() {
        let mut a = create_pd_string("Test");
        let b = create_pd_string("");
        
        unsafe {
            pd_string_append(&mut a, &b);
            assert_eq!(pd_string_to_rust(&a), "Test");
            
            free_pd_string(a);
            free_pd_string(b);
        }
    }

    #[test]
    fn test_string_append_to_empty() {
        let mut a = create_pd_string("");
        let b = create_pd_string("Test");
        
        unsafe {
            pd_string_append(&mut a, &b);
            assert_eq!(pd_string_to_rust(&a), "Test");
            
            free_pd_string(a);
            free_pd_string(b);
        }
    }

    #[test]
    fn test_string_append_multiple() {
        let mut a = create_pd_string("A");
        let b = create_pd_string("B");
        let c = create_pd_string("C");
        
        unsafe {
            pd_string_append(&mut a, &b);
            assert_eq!(pd_string_to_rust(&a), "AB");
            
            pd_string_append(&mut a, &c);
            assert_eq!(pd_string_to_rust(&a), "ABC");
            
            free_pd_string(a);
            free_pd_string(b);
            free_pd_string(c);
        }
    }

    #[test]
    fn test_int_to_string() {
        let result = pd_int_to_string(42);
        unsafe {
            assert_eq!(pd_string_to_rust(&result), "42");
            assert_eq!(result.len, 2);
            assert_eq!(result.capacity, 2);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_int_to_string_negative() {
        let result = pd_int_to_string(-42);
        unsafe {
            assert_eq!(pd_string_to_rust(&result), "-42");
            assert_eq!(result.len, 3);
            assert_eq!(result.capacity, 3);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_int_to_string_zero() {
        let result = pd_int_to_string(0);
        unsafe {
            assert_eq!(pd_string_to_rust(&result), "0");
            assert_eq!(result.len, 1);
            assert_eq!(result.capacity, 1);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_int_to_string_large_numbers() {
        let result = pd_int_to_string(i64::MAX);
        unsafe {
            assert_eq!(pd_string_to_rust(&result), i64::MAX.to_string());
            free_pd_string(result);
        }
        
        let result = pd_int_to_string(i64::MIN);
        unsafe {
            assert_eq!(pd_string_to_rust(&result), i64::MIN.to_string());
            free_pd_string(result);
        }
    }

    #[test]
    fn test_string_length() {
        let s = create_pd_string("Hello, World!");
        let len = unsafe { pd_string_length(&s) };
        assert_eq!(len, 13);
        
        unsafe { free_pd_string(s); }
    }

    #[test]
    fn test_string_length_empty() {
        let s = create_pd_string("");
        let len = unsafe { pd_string_length(&s) };
        assert_eq!(len, 0);
        
        unsafe { free_pd_string(s); }
    }

    #[test]
    fn test_string_length_unicode() {
        let s = create_pd_string("Hello 世界 🦀");
        let len = unsafe { pd_string_length(&s) };
        // Length in bytes, not characters
        assert_eq!(len, s.len);
        
        unsafe { free_pd_string(s); }
    }

    #[test]
    fn test_pd_string_fields() {
        let s = create_pd_string("Test String");
        
        assert_eq!(s.len, 11);
        assert_eq!(s.capacity, 11);
        assert!(!s.data.is_null());
        
        unsafe { free_pd_string(s); }
    }

    #[test]
    fn test_string_concat_long_strings() {
        let a = create_pd_string(&"A".repeat(1000));
        let b = create_pd_string(&"B".repeat(1000));
        
        let result = unsafe { pd_string_concat(&a, &b) };
        
        unsafe {
            let result_str = pd_string_to_rust(&result);
            assert_eq!(result_str.len(), 2000);
            assert!(result_str.starts_with("AAAA"));
            assert!(result_str.ends_with("BBBB"));
            assert_eq!(result.len, 2000);
            
            free_pd_string(a);
            free_pd_string(b);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_string_append_self_reference() {
        let mut a = create_pd_string("Hello");
        let original = unsafe { pd_string_to_rust(&a) };
        
        // Create a copy to append to itself (simulating self-reference scenario)
        let b = create_pd_string(&original);
        
        unsafe {
            pd_string_append(&mut a, &b);
            assert_eq!(pd_string_to_rust(&a), "HelloHello");
            
            free_pd_string(a);
            free_pd_string(b);
        }
    }

    #[test]
    fn test_null_termination() {
        // Verify that strings are properly null-terminated
        let s = create_pd_string("Test");
        
        unsafe {
            let c_str = CStr::from_ptr(s.data);
            assert_eq!(c_str.to_str().unwrap(), "Test");
            
            // Check that the byte after the string is null
            let bytes = std::slice::from_raw_parts(s.data as *const u8, (s.len + 1) as usize);
            assert_eq!(bytes[s.len as usize], 0);
            
            free_pd_string(s);
        }
    }

    #[test]
    fn test_int_to_string_consistency() {
        // Test a range of numbers to ensure consistency
        for i in -100..=100 {
            let result = pd_int_to_string(i);
            unsafe {
                assert_eq!(pd_string_to_rust(&result), i.to_string());
                free_pd_string(result);
            }
        }
    }

    #[test]
    fn test_string_operations_preserve_capacity() {
        let a = create_pd_string("Test");
        assert_eq!(a.len, a.capacity);
        
        let b = create_pd_string("123");
        let result = unsafe { pd_string_concat(&a, &b) };
        
        unsafe {
            // After concat, len should equal capacity for a fresh string
            assert_eq!(result.len, result.capacity);
            assert_eq!(result.len, 7);
            
            free_pd_string(a);
            free_pd_string(b);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_edge_case_single_char() {
        let a = create_pd_string("A");
        let b = create_pd_string("B");
        
        let result = unsafe { pd_string_concat(&a, &b) };
        unsafe {
            assert_eq!(pd_string_to_rust(&result), "AB");
            assert_eq!(result.len, 2);
            free_pd_string(a);
            free_pd_string(b);
            free_pd_string(result);
        }
    }

    #[test]
    fn test_memory_safety_repeated_operations() {
        // Stress test with multiple operations
        let mut strings = Vec::new();
        
        // Create multiple strings
        for i in 0..10 {
            strings.push(create_pd_string(&format!("String{}", i)));
        }
        
        // Concatenate them all
        let mut result = create_pd_string("");
        for s in &strings {
            unsafe {
                pd_string_append(&mut result, s);
            }
        }
        
        unsafe {
            let final_str = pd_string_to_rust(&result);
            assert!(final_str.contains("String0"));
            assert!(final_str.contains("String9"));
            
            // Clean up
            free_pd_string(result);
            for s in strings {
                free_pd_string(s);
            }
        }
    }
}
