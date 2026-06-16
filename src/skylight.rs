//! # SkyLight Private API Bindings
//!
//! Runtime bindings to Apple's private SkyLight framework, which provides the
//! low-level interface for managing macOS Spaces (virtual desktops), window
//! ordering, window-to-space associations, and other window-server operations.
//!
//! **These APIs are private and undocumented.** They are not covered by any
//! stability guarantee from Apple and may change or disappear across macOS
//! releases. Use at your own risk.
//!
//! The framework is loaded at runtime via `dlopen` / `dlsym` so that the
//! binary can degrade gracefully (or produce a clear error) if the symbols are
//! unavailable on a particular OS version.

use std::ffi::{CStr, CString};
use std::fmt;

use core_foundation_sys::array::CFArrayRef;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::string::CFStringRef;
use core_graphics_types::geometry::CGRect;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when loading or calling into the SkyLight framework.
#[derive(Debug)]
pub enum SkyLightError {
    /// The SkyLight framework dylib could not be loaded via `dlopen`.
    FrameworkNotFound,
    /// A specific symbol could not be resolved via `dlsym`.
    SymbolNotFound(String),
    /// A CoreGraphics error code returned by one of the SLS functions.
    CGError(i32),
}

impl fmt::Display for SkyLightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkyLightError::FrameworkNotFound => {
                write!(f, "failed to load SkyLight.framework (dlopen returned NULL)")
            }
            SkyLightError::SymbolNotFound(sym) => {
                write!(f, "symbol not found in SkyLight.framework: {sym}")
            }
            SkyLightError::CGError(code) => {
                write!(f, "CoreGraphics error code {code}")
            }
        }
    }
}

impl std::error::Error for SkyLightError {}

// ---------------------------------------------------------------------------
// Space type enum
// ---------------------------------------------------------------------------

/// The type of a macOS Space (virtual desktop).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SpaceType {
    /// A regular, user-created desktop space.
    User = 0,
    /// A fullscreen-app space.
    Fullscreen = 4,
    /// A system-level space (e.g. the dashboard or notification centre).
    System = 7,
}

impl SpaceType {
    /// Attempt to convert a raw `i32` value into a [`SpaceType`].
    pub fn from_raw(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::User),
            4 => Some(Self::Fullscreen),
            7 => Some(Self::System),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Function-pointer type aliases
// ---------------------------------------------------------------------------

// Connection
type FnSLSMainConnectionID = unsafe extern "C" fn() -> i32;

// Space queries
type FnSLSGetActiveSpace = unsafe extern "C" fn(cid: i32) -> u64;
type FnSLSCopyManagedDisplaySpaces = unsafe extern "C" fn(cid: i32) -> CFArrayRef;
type FnSLSCopyManagedDisplayForSpace = unsafe extern "C" fn(cid: i32, sid: u64) -> CFStringRef;
type FnSLSSpaceGetType = unsafe extern "C" fn(cid: i32, sid: u64) -> i32;
type FnSLSCopyManagedDisplays = unsafe extern "C" fn(cid: i32) -> CFArrayRef;

// Space creation / destruction
type FnSLSSpaceCreate = unsafe extern "C" fn(cid: i32, flags: i32, properties: CFDictionaryRef) -> u64;
type FnSLSSpaceDestroy = unsafe extern "C" fn(cid: i32, sid: u64);

// Space switching
type FnSLSSpaceSwitchToSpace = unsafe extern "C" fn(cid: i32, sid: u64);

// Window–Space association
type FnSLSMoveWindowsToManagedSpace =
    unsafe extern "C" fn(cid: i32, window_ids: CFArrayRef, sid: u64);
type FnSLSAddWindowsToSpaces =
    unsafe extern "C" fn(cid: i32, window_ids: CFArrayRef, space_ids: CFArrayRef);
type FnSLSRemoveWindowsFromSpaces =
    unsafe extern "C" fn(cid: i32, window_ids: CFArrayRef, space_ids: CFArrayRef);
type FnSLSCopySpacesForWindows =
    unsafe extern "C" fn(cid: i32, selector: i32, window_ids: CFArrayRef) -> CFArrayRef;

// Window properties
type FnSLSSetWindowAlpha = unsafe extern "C" fn(cid: i32, wid: u32, alpha: f32) -> i32;
type FnSLSSetWindowLevel = unsafe extern "C" fn(cid: i32, wid: u32, level: i32) -> i32;
type FnSLSOrderWindow =
    unsafe extern "C" fn(cid: i32, wid: u32, mode: i32, relative_wid: u32) -> i32;
type FnSLSGetWindowBounds =
    unsafe extern "C" fn(cid: i32, wid: u32, rect: *mut CGRect) -> i32;

// ---------------------------------------------------------------------------
// Symbol resolution macro
// ---------------------------------------------------------------------------

/// Resolve a single symbol from the loaded framework handle.
///
/// Expands to a block that calls `dlsym`, transmutes the resulting pointer to
/// the expected function-pointer type, and returns an
/// `Err(SkyLightError::SymbolNotFound)` if the symbol is NULL.
macro_rules! resolve_sym {
    ($handle:expr, $name:literal, $fn_ty:ty) => {{
        let cname = CStr::from_bytes_with_nul(concat!($name, "\0").as_bytes())
            .expect("symbol name should be a valid C string");
        let ptr = unsafe { libc::dlsym($handle, cname.as_ptr()) };
        if ptr.is_null() {
            return Err(SkyLightError::SymbolNotFound($name.to_owned()));
        }
        unsafe { std::mem::transmute::<*mut libc::c_void, $fn_ty>(ptr) }
    }};
}

// ---------------------------------------------------------------------------
// SkyLight struct
// ---------------------------------------------------------------------------

/// Handle to the loaded SkyLight framework with resolved function pointers.
///
/// Create an instance via [`SkyLight::new()`], which loads the framework and
/// resolves every required symbol up-front. All subsequent method calls are
/// thin wrappers that invoke the corresponding C function pointer.
pub struct SkyLight {
    // We keep the dlopen handle alive for the lifetime of this struct so that
    // the resolved function pointers remain valid.
    _handle: *mut libc::c_void,

    // -- Connection -----------------------------------------------------------
    sls_main_connection_id: FnSLSMainConnectionID,

    // -- Space queries --------------------------------------------------------
    sls_get_active_space: FnSLSGetActiveSpace,
    sls_copy_managed_display_spaces: FnSLSCopyManagedDisplaySpaces,
    sls_copy_managed_display_for_space: FnSLSCopyManagedDisplayForSpace,
    sls_space_get_type: FnSLSSpaceGetType,
    sls_copy_managed_displays: FnSLSCopyManagedDisplays,

    // -- Space creation / destruction -----------------------------------------
    sls_space_create: FnSLSSpaceCreate,
    sls_space_destroy: FnSLSSpaceDestroy,

    // -- Space switching ------------------------------------------------------
    sls_space_switch_to_space: Option<FnSLSSpaceSwitchToSpace>,

    // -- Window–Space association ---------------------------------------------
    sls_move_windows_to_managed_space: FnSLSMoveWindowsToManagedSpace,
    sls_add_windows_to_spaces: FnSLSAddWindowsToSpaces,
    sls_remove_windows_from_spaces: FnSLSRemoveWindowsFromSpaces,
    sls_copy_spaces_for_windows: FnSLSCopySpacesForWindows,

    // -- Window properties ----------------------------------------------------
    sls_set_window_alpha: FnSLSSetWindowAlpha,
    sls_set_window_level: FnSLSSetWindowLevel,
    sls_order_window: FnSLSOrderWindow,
    sls_get_window_bounds: FnSLSGetWindowBounds,
}

// The raw handle is not Send/Sync by default, but the dlopen handle and the
// resolved function pointers are safe to share across threads — the window
// server itself serialises access internally.
unsafe impl Send for SkyLight {}
unsafe impl Sync for SkyLight {}

/// Path to the SkyLight private framework on macOS.
const SKYLIGHT_PATH: &str =
    "/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";

impl SkyLight {
    /// Load the SkyLight framework and resolve all required symbols.
    ///
    /// # Errors
    ///
    /// Returns [`SkyLightError::FrameworkNotFound`] if the framework cannot be
    /// loaded, or [`SkyLightError::SymbolNotFound`] if any expected symbol is
    /// missing from the loaded binary.
    pub fn new() -> Result<Self, SkyLightError> {
        let path = CString::new(SKYLIGHT_PATH).expect("framework path should be a valid C string");

        // RTLD_LAZY: resolve symbols on first use (we transmute immediately, so
        // effectively eager). RTLD_LOCAL: keep symbols private.
        let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
        if handle.is_null() {
            return Err(SkyLightError::FrameworkNotFound);
        }

        Ok(Self {
            _handle: handle,

            // Connection
            sls_main_connection_id: resolve_sym!(handle, "SLSMainConnectionID", FnSLSMainConnectionID),

            // Space queries
            sls_get_active_space: resolve_sym!(handle, "SLSGetActiveSpace", FnSLSGetActiveSpace),
            sls_copy_managed_display_spaces: resolve_sym!(handle, "SLSCopyManagedDisplaySpaces", FnSLSCopyManagedDisplaySpaces),
            sls_copy_managed_display_for_space: resolve_sym!(handle, "SLSCopyManagedDisplayForSpace", FnSLSCopyManagedDisplayForSpace),
            sls_space_get_type: resolve_sym!(handle, "SLSSpaceGetType", FnSLSSpaceGetType),
            sls_copy_managed_displays: resolve_sym!(handle, "SLSCopyManagedDisplays", FnSLSCopyManagedDisplays),

            // Space creation / destruction
            sls_space_create: resolve_sym!(handle, "SLSSpaceCreate", FnSLSSpaceCreate),
            sls_space_destroy: resolve_sym!(handle, "SLSSpaceDestroy", FnSLSSpaceDestroy),

            // Space switching
            sls_space_switch_to_space: unsafe {
                let cname = std::ffi::CStr::from_bytes_with_nul(b"SLSSpaceSwitchToSpace\0").unwrap();
                let ptr = libc::dlsym(handle, cname.as_ptr());
                if ptr.is_null() {
                    None
                } else {
                    Some(std::mem::transmute::<*mut libc::c_void, FnSLSSpaceSwitchToSpace>(ptr))
                }
            },

            // Window–Space association
            sls_move_windows_to_managed_space: resolve_sym!(handle, "SLSMoveWindowsToManagedSpace", FnSLSMoveWindowsToManagedSpace),
            sls_add_windows_to_spaces: resolve_sym!(handle, "SLSAddWindowsToSpaces", FnSLSAddWindowsToSpaces),
            sls_remove_windows_from_spaces: resolve_sym!(handle, "SLSRemoveWindowsFromSpaces", FnSLSRemoveWindowsFromSpaces),
            sls_copy_spaces_for_windows: resolve_sym!(handle, "SLSCopySpacesForWindows", FnSLSCopySpacesForWindows),

            // Window properties
            sls_set_window_alpha: resolve_sym!(handle, "SLSSetWindowAlpha", FnSLSSetWindowAlpha),
            sls_set_window_level: resolve_sym!(handle, "SLSSetWindowLevel", FnSLSSetWindowLevel),
            sls_order_window: resolve_sym!(handle, "SLSOrderWindow", FnSLSOrderWindow),
            sls_get_window_bounds: resolve_sym!(handle, "SLSGetWindowBounds", FnSLSGetWindowBounds),
        })
    }

    // -----------------------------------------------------------------------
    // Connection
    // -----------------------------------------------------------------------

    /// Obtain the main connection ID to the window server.
    ///
    /// This is the `cid` value required by almost every other SkyLight call.
    pub fn main_connection_id(&self) -> i32 {
        unsafe { (self.sls_main_connection_id)() }
    }

    // -----------------------------------------------------------------------
    // Space queries
    // -----------------------------------------------------------------------

    /// Return the space ID of the currently active (focused) space.
    pub fn get_active_space(&self, cid: i32) -> u64 {
        unsafe { (self.sls_get_active_space)(cid) }
    }

    /// Return a `CFArray` describing every managed display and its spaces.
    ///
    /// The caller owns the returned array and is responsible for releasing it.
    pub fn copy_managed_display_spaces(&self, cid: i32) -> CFArrayRef {
        unsafe { (self.sls_copy_managed_display_spaces)(cid) }
    }

    /// Return the `CFString` UUID of the display that owns the given space.
    ///
    /// The caller owns the returned string and is responsible for releasing it.
    pub fn copy_managed_display_for_space(&self, cid: i32, sid: u64) -> CFStringRef {
        unsafe { (self.sls_copy_managed_display_for_space)(cid, sid) }
    }

    /// Return the raw type code for the given space.
    ///
    /// Use [`SpaceType::from_raw`] to convert the result into the typed enum.
    pub fn space_get_type(&self, cid: i32, sid: u64) -> i32 {
        unsafe { (self.sls_space_get_type)(cid, sid) }
    }

    /// Return a `CFArray` of display UUID strings for all managed displays.
    ///
    /// The caller owns the returned array and is responsible for releasing it.
    pub fn copy_managed_displays(&self, cid: i32) -> CFArrayRef {
        unsafe { (self.sls_copy_managed_displays)(cid) }
    }

    // -----------------------------------------------------------------------
    // Space creation / destruction
    // -----------------------------------------------------------------------

    /// Create a new space with the given flags and optional properties.
    ///
    /// Returns the space ID of the newly created space.
    pub fn space_create(
        &self,
        cid: i32,
        flags: i32,
        properties: CFDictionaryRef,
    ) -> u64 {
        unsafe { (self.sls_space_create)(cid, flags, properties) }
    }

    /// Destroy a previously created space.
    pub fn space_destroy(&self, cid: i32, sid: u64) {
        unsafe { (self.sls_space_destroy)(cid, sid) }
    }

    // -----------------------------------------------------------------------
    // Space switching
    // -----------------------------------------------------------------------

    /// Switch the user's view to the specified space.
    pub fn space_switch_to_space(&self, cid: i32, sid: u64) {
        if let Some(f) = self.sls_space_switch_to_space {
            unsafe { f(cid, sid) }
        } else {
            log::warn!("SLSSpaceSwitchToSpace not available");
        }
    }

    // -----------------------------------------------------------------------
    // Window–Space association
    // -----------------------------------------------------------------------

    /// Move the given windows to a single managed space.
    ///
    /// * `window_ids` – `CFArray` of `CGWindowID` (`u32`) values.
    /// * `sid` – target space ID.
    pub fn move_windows_to_managed_space(
        &self,
        cid: i32,
        window_ids: CFArrayRef,
        sid: u64,
    ) {
        unsafe { (self.sls_move_windows_to_managed_space)(cid, window_ids, sid) }
    }

    /// Add the given windows to one or more spaces (making them visible on all
    /// specified spaces simultaneously).
    ///
    /// * `window_ids` – `CFArray` of `CGWindowID` values.
    /// * `space_ids` – `CFArray` of space ID (`u64`) values.
    pub fn add_windows_to_spaces(
        &self,
        cid: i32,
        window_ids: CFArrayRef,
        space_ids: CFArrayRef,
    ) {
        unsafe { (self.sls_add_windows_to_spaces)(cid, window_ids, space_ids) }
    }

    /// Remove the given windows from one or more spaces.
    ///
    /// * `window_ids` – `CFArray` of `CGWindowID` values.
    /// * `space_ids` – `CFArray` of space ID (`u64`) values.
    pub fn remove_windows_from_spaces(
        &self,
        cid: i32,
        window_ids: CFArrayRef,
        space_ids: CFArrayRef,
    ) {
        unsafe { (self.sls_remove_windows_from_spaces)(cid, window_ids, space_ids) }
    }

    /// Return the spaces that the given windows belong to.
    ///
    /// * `selector` – a bitmask controlling which space types to query.
    /// * `window_ids` – `CFArray` of `CGWindowID` values.
    ///
    /// The caller owns the returned array and is responsible for releasing it.
    pub fn copy_spaces_for_windows(
        &self,
        cid: i32,
        selector: i32,
        window_ids: CFArrayRef,
    ) -> CFArrayRef {
        unsafe { (self.sls_copy_spaces_for_windows)(cid, selector, window_ids) }
    }

    // -----------------------------------------------------------------------
    // Window properties
    // -----------------------------------------------------------------------

    /// Set the alpha (opacity) of a window.
    ///
    /// `alpha` should be in the range `0.0` (fully transparent) to `1.0`
    /// (fully opaque).
    ///
    /// # Errors
    ///
    /// Returns [`SkyLightError::CGError`] on failure.
    pub fn set_window_alpha(
        &self,
        cid: i32,
        wid: u32,
        alpha: f32,
    ) -> Result<(), SkyLightError> {
        let err = unsafe { (self.sls_set_window_alpha)(cid, wid, alpha) };
        cg_result(err)
    }

    /// Set the window level (z-ordering layer) of a window.
    ///
    /// # Errors
    ///
    /// Returns [`SkyLightError::CGError`] on failure.
    pub fn set_window_level(
        &self,
        cid: i32,
        wid: u32,
        level: i32,
    ) -> Result<(), SkyLightError> {
        let err = unsafe { (self.sls_set_window_level)(cid, wid, level) };
        cg_result(err)
    }

    /// Reorder a window relative to another window.
    ///
    /// * `mode` – ordering mode (e.g. above, below, out).
    /// * `relative_wid` – the window to order relative to, or `0` for none.
    ///
    /// # Errors
    ///
    /// Returns [`SkyLightError::CGError`] on failure.
    pub fn order_window(
        &self,
        cid: i32,
        wid: u32,
        mode: i32,
        relative_wid: u32,
    ) -> Result<(), SkyLightError> {
        let err = unsafe { (self.sls_order_window)(cid, wid, mode, relative_wid) };
        cg_result(err)
    }

    /// Retrieve the current frame rectangle of a window.
    ///
    /// # Errors
    ///
    /// Returns [`SkyLightError::CGError`] on failure.
    pub fn get_window_bounds(
        &self,
        cid: i32,
        wid: u32,
    ) -> Result<CGRect, SkyLightError> {
        let mut rect = CGRect::new(
            &core_graphics_types::geometry::CGPoint::new(0.0, 0.0),
            &core_graphics_types::geometry::CGSize::new(0.0, 0.0),
        );
        let err = unsafe { (self.sls_get_window_bounds)(cid, wid, &mut rect) };
        cg_result(err).map(|()| rect)
    }
}

impl Drop for SkyLight {
    fn drop(&mut self) {
        // Balance the dlopen reference count.
        unsafe {
            libc::dlclose(self._handle);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a CoreGraphics error code to a `Result`.
///
/// `kCGErrorSuccess` is `0`; any other value is treated as an error.
fn cg_result(code: i32) -> Result<(), SkyLightError> {
    if code == 0 {
        Ok(())
    } else {
        Err(SkyLightError::CGError(code))
    }
}
