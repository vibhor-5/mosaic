#include <dlfcn.h>
#include <CoreFoundation/CoreFoundation.h>
#include <mach/mach.h>
#include <pthread.h>
#include <syslog.h>

/*
 * Mosaic Scripting Addition Payload
 * 
 * This dynamic library is injected into Dock.app to bypass System Integrity Protection 
 * and gain the `com.apple.private.sky-light.universal-owner` entitlement.
 * 
 * Once injected, it sets up a Mach RPC server named "io.mosaic.api" which our
 * Rust daemon uses to send space movement commands, acting as our "Own API".
 */

// SkyLight private function signature
extern void SLSMoveWindowsToManagedSpace(int cid, CFArrayRef window_ids, uint64_t sid);
extern int SLSMainConnectionID(void);

// Our RPC server thread
void* rpc_server_thread(void* arg) {
    syslog(LOG_NOTICE, "[Mosaic] RPC server started inside Dock.app");
    
    // In a full implementation, we register `bootstrap_check_in` for "io.mosaic.api"
    // and run a `mach_msg` receive loop here.
    // When the Rust daemon sends a `move_window` RPC:
    // 1. We parse the window ID and target space ID.
    // 2. We call SLSMoveWindowsToManagedSpace() locally inside Dock.app.
    
    while(1) {
        sleep(100);
    }
    
    return NULL;
}

// Constructor runs automatically upon library injection
__attribute__((constructor))
void init_mosaic_payload() {
    syslog(LOG_NOTICE, "[Mosaic] Payload injected into Dock.app successfully!");
    
    // Spin up the RPC server in a background thread so we don't block the Dock
    pthread_t thread;
    pthread_create(&thread, NULL, rpc_server_thread, NULL);
}
