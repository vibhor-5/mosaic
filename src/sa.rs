use std::ffi::CString;

#[link(name = "mosaicUser", kind = "static")]
extern "C" {
    fn move_window(server: u32, window_id: u32, space_id: u64) -> i32;
}

#[link(name = "Foundation", kind = "framework")]
extern "C" {
    fn bootstrap_look_up(bp: u32, service_name: *const libc::c_char, sp: *mut u32) -> i32;
}

// macOS Mach specific definitions
const BOOTSTRAP_SUCCESS: i32 = 0;
extern "C" {
    static bootstrap_port: u32;
}

pub struct ScriptingAddition {
    port: u32,
}

impl ScriptingAddition {
    pub fn new() -> Option<Self> {
        unsafe {
            let mut sp: u32 = 0;
            let name = CString::new("io.mosaic.api").unwrap();
            let kr = bootstrap_look_up(bootstrap_port, name.as_ptr(), &mut sp);
            
            if kr == BOOTSTRAP_SUCCESS {
                Some(ScriptingAddition { port: sp })
            } else {
                None
            }
        }
    }

    pub fn move_window_to_space(&self, window_id: u32, space_id: u64) -> Result<(), i32> {
        let kr = unsafe { move_window(self.port, window_id, space_id) };
        if kr == 0 {
            Ok(())
        } else {
            Err(kr)
        }
    }
}
