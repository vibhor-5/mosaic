//! Accessibility API wrapper module for Mosaic.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFStringRef;
use core_foundation::array::CFArrayRef;
use core_foundation::dictionary::CFDictionaryRef;
use std::os::raw::{c_void, c_int};

pub type AXUIElementRef = *mut c_void;
pub type AXObserverRef = *mut c_void;
pub type AXError = i32;

// AXError constants
pub const K_AX_ERROR_SUCCESS: AXError = 0;
pub const K_AX_ERROR_ACTION_UNSUPPORTED: AXError = -25201;
pub const K_AX_ERROR_API_DISABLED: AXError = -25211;
pub const K_AX_ERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25205;
pub const K_AX_ERROR_CANNOT_COMPLETE: AXError = -25204;
pub const K_AX_ERROR_FAILURE: AXError = -25200;
pub const K_AX_ERROR_ILLEGAL_ARGUMENT: AXError = -25203;
pub const K_AX_ERROR_INVALID_UI_ELEMENT: AXError = -25202;
pub const K_AX_ERROR_INVALID_UI_ELEMENT_OBSERVER: AXError = -25207;
pub const K_AX_ERROR_NOT_ENOUGH_PRECISION: AXError = -25208;
pub const K_AX_ERROR_NOT_IMPLEMENTED: AXError = -25206;
pub const K_AX_ERROR_NOTIFICATION_ALREADY_REGISTERED: AXError = -25209;
pub const K_AX_ERROR_NOTIFICATION_NOT_REGISTERED: AXError = -25210;
pub const K_AX_ERROR_NOTIFICATION_UNSUPPORTED: AXError = -25212;
pub const K_AX_ERROR_PARAMETERIZED_ATTRIBUTE_UNSUPPORTED: AXError = -25213;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub fn AXUIElementCreateApplication(pid: c_int) -> AXUIElementRef;
    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementCopyAttributeValue(element: AXUIElementRef, attribute: CFStringRef, value: *mut CFTypeRef) -> AXError;
    pub fn AXUIElementSetAttributeValue(element: AXUIElementRef, attribute: CFStringRef, value: CFTypeRef) -> AXError;
    pub fn AXUIElementCopyAttributeNames(element: AXUIElementRef, names: *mut CFArrayRef) -> AXError;
    pub fn AXUIElementIsAttributeSettable(element: AXUIElementRef, attribute: CFStringRef, settable: *mut bool) -> AXError;
    pub fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;
    pub fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut c_int) -> AXError;
    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;

    pub fn AXObserverCreate(pid: c_int, callback: *mut c_void, observer: *mut AXObserverRef) -> AXError;
    pub fn AXObserverAddNotification(observer: AXObserverRef, element: AXUIElementRef, notification: CFStringRef, refcon: *mut c_void) -> AXError;
    pub fn AXObserverRemoveNotification(observer: AXObserverRef, element: AXUIElementRef, notification: CFStringRef) -> AXError;
    pub fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFTypeRef; // returns CFRunLoopSourceRef
}

pub struct AXElement(pub AXUIElementRef);

impl Drop for AXElement {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0 as CFTypeRef);
            }
        }
    }
}

impl Clone for AXElement {
    fn clone(&self) -> Self {
        unsafe {
            if !self.0.is_null() {
                core_foundation::base::CFRetain(self.0 as CFTypeRef);
            }
        }
        AXElement(self.0)
    }
}

impl AXElement {
    pub fn application(pid: i32) -> Result<Self, AXError> {
        let el = unsafe { AXUIElementCreateApplication(pid as c_int) };
        if el.is_null() {
            Err(K_AX_ERROR_FAILURE)
        } else {
            Ok(AXElement(el))
        }
    }

    pub fn system_wide() -> Result<Self, AXError> {
        let el = unsafe { AXUIElementCreateSystemWide() };
        if el.is_null() {
            Err(K_AX_ERROR_FAILURE)
        } else {
            Ok(AXElement(el))
        }
    }

    pub fn get_attribute<T: TCFType>(&self, name: &str) -> Result<T, AXError> {
        let attr_name = core_foundation::string::CFString::new(name);
        let mut value: CFTypeRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(self.0, attr_name.as_concrete_TypeRef(), &mut value) };
        if err == K_AX_ERROR_SUCCESS && !value.is_null() {
            unsafe {
                Ok(T::wrap_under_create_rule(std::mem::transmute_copy(&value)))
            }
        } else {
            Err(err)
        }
    }

    pub fn set_attribute<T: TCFType>(&self, name: &str, value: &T) -> Result<(), AXError> {
        let attr_name = core_foundation::string::CFString::new(name);
        let err = unsafe { AXUIElementSetAttributeValue(self.0, attr_name.as_concrete_TypeRef(), value.as_CFTypeRef()) };
        if err == K_AX_ERROR_SUCCESS {
            Ok(())
        } else {
            Err(err)
        }
    }

    pub fn get_windows(&self) -> Result<Vec<AXElement>, AXError> {
        match self.get_attribute::<core_foundation::array::CFArray>("AXWindows") {
            Ok(array) => {
                let mut windows = Vec::new();
                for i in 0..array.len() {
                    let val = unsafe { core_foundation::array::CFArrayGetValueAtIndex(array.as_concrete_TypeRef(), i) };
                    if !val.is_null() {
                        unsafe {
                            core_foundation::base::CFRetain(val);
                        }
                        windows.push(AXElement(val as AXUIElementRef));
                    }
                }
                Ok(windows)
            }
            Err(e) => Err(e),
        }
    }

    pub fn get_position(&self) -> Result<(f64, f64), AXError> {
        // Complex because AXPosition is an AXValue, requires decoding.
        // For simplicity, we just return an error or default if we don't implement full AXValue parsing.
        Err(K_AX_ERROR_NOT_IMPLEMENTED)
    }

    pub fn set_position(&self, _x: f64, _y: f64) -> Result<(), AXError> {
        Err(K_AX_ERROR_NOT_IMPLEMENTED)
    }

    pub fn get_size(&self) -> Result<(f64, f64), AXError> {
        Err(K_AX_ERROR_NOT_IMPLEMENTED)
    }

    pub fn set_size(&self, _w: f64, _h: f64) -> Result<(), AXError> {
        Err(K_AX_ERROR_NOT_IMPLEMENTED)
    }

    pub fn get_title(&self) -> Result<String, AXError> {
        match self.get_attribute::<core_foundation::string::CFString>("AXTitle") {
            Ok(s) => Ok(s.to_string()),
            Err(e) => Err(e),
        }
    }

    pub fn is_minimized(&self) -> Result<bool, AXError> {
        match self.get_attribute::<core_foundation::boolean::CFBoolean>("AXMinimized") {
            Ok(b) => Ok(b.into()),
            Err(_) => Ok(false), // Fallback
        }
    }

    pub fn get_role(&self) -> Result<String, AXError> {
        match self.get_attribute::<core_foundation::string::CFString>("AXRole") {
            Ok(s) => Ok(s.to_string()),
            Err(e) => Err(e),
        }
    }

    pub fn get_subrole(&self) -> Result<String, AXError> {
        match self.get_attribute::<core_foundation::string::CFString>("AXSubrole") {
            Ok(s) => Ok(s.to_string()),
            Err(e) => Err(e),
        }
    }

    pub fn perform_action(&self, action: &str) -> Result<(), AXError> {
        let action_name = core_foundation::string::CFString::new(action);
        let err = unsafe { AXUIElementPerformAction(self.0, action_name.as_concrete_TypeRef()) };
        if err == K_AX_ERROR_SUCCESS {
            Ok(())
        } else {
            Err(err)
        }
    }

    pub fn pid(&self) -> Result<i32, AXError> {
        let mut pid: c_int = 0;
        let err = unsafe { AXUIElementGetPid(self.0, &mut pid) };
        if err == K_AX_ERROR_SUCCESS {
            Ok(pid as i32)
        } else {
            Err(err)
        }
    }
}

pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn request_trust() -> bool {
    use core_foundation::string::CFString;
    use core_foundation::boolean::CFBoolean;

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let val = CFBoolean::true_value();
    
    // Create dictionary using raw CoreFoundation to avoid complex setup
    unsafe {
        let keys = [key.as_CFTypeRef()];
        let values = [val.as_CFTypeRef()];
        let dict = core_foundation::dictionary::CFDictionaryCreate(
            core_foundation::base::kCFAllocatorDefault,
            keys.as_ptr() as *const *const c_void,
            values.as_ptr() as *const *const c_void,
            1,
            &core_foundation::dictionary::kCFTypeDictionaryKeyCallBacks,
            &core_foundation::dictionary::kCFTypeDictionaryValueCallBacks,
        );
        let result = AXIsProcessTrustedWithOptions(dict as CFDictionaryRef);
        CFRelease(dict as CFTypeRef);
        result
    }
}

pub struct AXObserver(pub AXObserverRef);

unsafe impl Send for AXObserver {}
unsafe impl Sync for AXObserver {}

impl Drop for AXObserver {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0 as CFTypeRef);
            }
        }
    }
}

pub type AXObserverCallback = extern "C" fn(AXObserverRef, AXUIElementRef, CFStringRef, *mut c_void);

impl AXObserver {
    pub fn new(pid: i32, callback: AXObserverCallback) -> Result<Self, AXError> {
        let mut obs: AXObserverRef = std::ptr::null_mut();
        let err = unsafe { AXObserverCreate(pid as c_int, callback as *mut c_void, &mut obs) };
        if err == K_AX_ERROR_SUCCESS && !obs.is_null() {
            Ok(AXObserver(obs))
        } else {
            Err(err)
        }
    }

    pub fn add_notification(&self, element: &AXElement, notification: &str, refcon: *mut c_void) -> Result<(), AXError> {
        let notif_name = core_foundation::string::CFString::new(notification);
        let err = unsafe { AXObserverAddNotification(self.0, element.0, notif_name.as_concrete_TypeRef(), refcon) };
        if err == K_AX_ERROR_SUCCESS {
            Ok(())
        } else {
            Err(err)
        }
    }

    pub fn remove_notification(&self, element: &AXElement, notification: &str) -> Result<(), AXError> {
        let notif_name = core_foundation::string::CFString::new(notification);
        let err = unsafe { AXObserverRemoveNotification(self.0, element.0, notif_name.as_concrete_TypeRef()) };
        if err == K_AX_ERROR_SUCCESS {
            Ok(())
        } else {
            Err(err)
        }
    }

    pub fn attach_to_run_loop(&self) {
        unsafe {
            let rl_source = AXObserverGetRunLoopSource(self.0);
            if !rl_source.is_null() {
                core_foundation::runloop::CFRunLoopAddSource(
                    core_foundation::runloop::CFRunLoopGetMain(),
                    rl_source as _,
                    core_foundation::runloop::kCFRunLoopDefaultMode,
                );
            }
        }
    }
}
