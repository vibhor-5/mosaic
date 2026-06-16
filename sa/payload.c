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

#include "mosaic.h"

// SkyLight private function signature
// Implementation of the MIG routine defined in mosaic.defs
kern_return_t move_window(
    mach_port_t server,
    uint32_t window_id,
    uint64_t space_id
) {
    syslog(LOG_NOTICE, "[Mosaic] Received RPC to move window %u to space %llu", window_id, space_id);
    
    void* handle = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY);
    if (!handle) return KERN_FAILURE;
    
    int (*SLSMainConnectionID)(void) = dlsym(handle, "SLSMainConnectionID");
    void (*SLSMoveWindowsToManagedSpace)(int, CFArrayRef, uint64_t) = dlsym(handle, "SLSMoveWindowsToManagedSpace");
    
    if (!SLSMainConnectionID || !SLSMoveWindowsToManagedSpace) return KERN_FAILURE;
    
    int cid = SLSMainConnectionID();
    
    // Create CFArray with the window ID
    uint32_t wid = window_id;
    CFNumberRef num = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &wid);
    const void* values[] = { num };
    CFArrayRef window_array = CFArrayCreate(kCFAllocatorDefault, values, 1, &kCFTypeArrayCallBacks);
    
    // Call SkyLight
    SLSMoveWindowsToManagedSpace(cid, window_array, space_id);
    
    CFRelease(window_array);
    CFRelease(num);
    
    return KERN_SUCCESS;
}

// Our RPC server thread
void* rpc_server_thread(void* arg) {
    syslog(LOG_NOTICE, "[Mosaic] RPC server started inside Dock.app");
    
    mach_port_t server_port;
    kern_return_t kr = mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &server_port);
    if (kr != KERN_SUCCESS) return NULL;
    
    kr = mach_port_insert_right(mach_task_self(), server_port, server_port, MACH_MSG_TYPE_MAKE_SEND);
    if (kr != KERN_SUCCESS) return NULL;
    
    // Register with bootstrap server
    mach_port_t bs_port;
    task_get_bootstrap_port(mach_task_self(), &bs_port);
    // bootstrap_register(bs_port, "io.mosaic.api", server_port); 
    // In a full implementation, we'd use bootstrap_check_in or bootstrap_register.
    // We'll simulate the mach_msg receive loop:
    extern boolean_t mosaic_server(mach_msg_header_t *, mach_msg_header_t *);
    mach_msg_server(mosaic_server, 1024, server_port, 0);
    
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
