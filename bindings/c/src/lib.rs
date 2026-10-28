//! C ownership: every returned string must be freed with dictation_string_free.
use std::{
    ffi::{CStr, CString, c_char},
    path::Path,
};
pub struct DictationConnection {
    connection: dictation_api::Connection,
}
fn string(value: String) -> *mut c_char {
    CString::new(value)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}
unsafe fn text<'a>(value: *const c_char) -> Result<&'a str, String> {
    if value.is_null() {
        return Err("null string".into());
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map_err(|e| e.to_string())
}
/// Connect to the per-user broker. A null endpoint selects the default path.
/// On failure, writes an allocated message to error when error is non-null.
/// # Safety
/// Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
/// Connection pointers must come from this library and remain open until the call returns.
/// The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dictation_connect(
    endpoint: *const c_char,
    error: *mut *mut c_char,
) -> *mut DictationConnection {
    if !error.is_null() {
        unsafe {
            *error = std::ptr::null_mut();
        }
    }
    let result = (|| {
        let endpoint = if endpoint.is_null() {
            None
        } else {
            Some(unsafe { text(endpoint) }?)
        };
        dictation_api::Connection::connect(endpoint.map(Path::new)).map_err(|e| e.to_string())
    })();
    match result {
        Ok(connection) => Box::into_raw(Box::new(DictationConnection { connection })),
        Err(e) => {
            if !error.is_null() {
                unsafe {
                    *error = string(e);
                }
            }
            std::ptr::null_mut()
        }
    }
}
/// Send a protocol Request encoded as JSON. The response or error is returned as JSON.
/// # Safety
/// Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
/// Connection pointers must come from this library and remain open until the call returns.
/// The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dictation_request(
    connection: *const DictationConnection,
    json: *const c_char,
    error: *mut *mut c_char,
) -> *mut c_char {
    if !error.is_null() {
        unsafe {
            *error = std::ptr::null_mut();
        }
    }
    let result = (|| {
        let connection = unsafe { connection.as_ref() }.ok_or("null connection")?;
        connection
            .connection
            .request_json(unsafe { text(json) }?)
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(value) => string(value),
        Err(e) => {
            if !error.is_null() {
                unsafe {
                    *error = string(e);
                }
            }
            std::ptr::null_mut()
        }
    }
}
/// Wait for an event. Null without an error means the wait timed out.
/// # Safety
/// Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
/// Connection pointers must come from this library and remain open until the call returns.
/// The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dictation_next_event(
    connection: *const DictationConnection,
    timeout_ms: u32,
    error: *mut *mut c_char,
) -> *mut c_char {
    if !error.is_null() {
        unsafe {
            *error = std::ptr::null_mut();
        }
    }
    let result = unsafe { connection.as_ref() }
        .ok_or_else(|| "null connection".to_string())
        .and_then(|c| {
            c.connection
                .next_event_json(timeout_ms)
                .map_err(|e| e.to_string())
        });
    match result {
        Ok(Some(value)) => string(value),
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            if !error.is_null() {
                unsafe {
                    *error = string(e);
                }
            }
            std::ptr::null_mut()
        }
    }
}
/// # Safety
/// The pointer must be null or an open connection returned by this library. Close each connection once; no call may use it concurrently with close.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dictation_close(connection: *mut DictationConnection) {
    if !connection.is_null() {
        drop(unsafe { Box::from_raw(connection) });
    }
}
/// # Safety
/// The pointer must be null or an allocated string returned by this library. Free each string exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dictation_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

/// Borrowed metadata, copied during client registration. Null icon means no icon.
#[repr(C)]
pub struct DictationApplication {
    pub id: *const c_char,
    pub name: *const c_char,
    pub icon: *const c_char,
    pub process_id: u32,
}
unsafe fn connect_registered(
    endpoint: *const c_char,
    error: *mut *mut c_char,
    request: dictation_api::Request,
) -> *mut DictationConnection {
    let connection = unsafe { dictation_connect(endpoint, error) };
    if connection.is_null() {
        return connection;
    }
    if let Err(problem) = unsafe { &*connection }.connection.request(request) {
        unsafe {
            dictation_close(connection);
        }
        if !error.is_null() {
            unsafe {
                *error = string(problem.to_string());
            }
        }
        return std::ptr::null_mut();
    }
    connection
}
/// Connect as a receiving application. Metadata and endpoint strings are borrowed.
/// # Safety
/// Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
/// Connection pointers must come from this library and remain open until the call returns.
/// The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dictation_client_connect(
    application: *const DictationApplication,
    endpoint: *const c_char,
    error: *mut *mut c_char,
) -> *mut DictationConnection {
    if !error.is_null() {
        unsafe {
            *error = std::ptr::null_mut();
        }
    }
    let application = (|| -> Result<dictation_api::Application, String> {
        let app = unsafe { application.as_ref() }.ok_or("null application")?;
        Ok(dictation_api::Application {
            id: unsafe { text(app.id) }?.into(),
            name: unsafe { text(app.name) }?.into(),
            icon: if app.icon.is_null() {
                None
            } else {
                Some(unsafe { text(app.icon) }?.into())
            },
            process_id: app.process_id,
        })
    })();
    match application {
        Ok(application) => unsafe {
            connect_registered(
                endpoint,
                error,
                dictation_api::Request::RegisterClient {
                    application,
                    processing: true,
                },
            )
        },
        Err(problem) => {
            if !error.is_null() {
                unsafe {
                    *error = string(problem);
                }
            }
            std::ptr::null_mut()
        }
    }
}
/// Connect as a dictation provider. The name and endpoint strings are borrowed.
/// # Safety
/// Non-null pointers must be valid for this call. Strings must be NUL-terminated UTF-8.
/// Connection pointers must come from this library and remain open until the call returns.
/// The optional error pointer must reference writable storage. Free returned strings with dictation_string_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dictation_provider_connect(
    name: *const c_char,
    endpoint: *const c_char,
    error: *mut *mut c_char,
) -> *mut DictationConnection {
    if !error.is_null() {
        unsafe {
            *error = std::ptr::null_mut();
        }
    }
    match unsafe { text(name) } {
        Ok(name) => unsafe {
            connect_registered(
                endpoint,
                error,
                dictation_api::Request::RegisterProvider { name: name.into() },
            )
        },
        Err(problem) => {
            if !error.is_null() {
                unsafe {
                    *error = string(problem);
                }
            }
            std::ptr::null_mut()
        }
    }
}
