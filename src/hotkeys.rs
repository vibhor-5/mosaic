use std::sync::{Arc, Mutex};
use global_hotkey::{GlobalHotKeyManager, GlobalHotKeyEvent, hotkey::{HotKey, Modifiers, Code}};
use crate::Mosaic;
use crate::layout::LayoutMode;

#[derive(Debug, PartialEq)]
enum KeyMode {
    Main,
    Resize,
}

pub fn start_hotkey_listener(state: Arc<Mutex<Mosaic>>) {
    let manager = GlobalHotKeyManager::new().unwrap();
    
    // -- Main Mode Hotkeys --
    let h_bsp = HotKey::new(Some(Modifiers::ALT), Code::KeyE);
    let h_monocle = HotKey::new(Some(Modifiers::ALT), Code::KeyF);
    
    let h_west = HotKey::new(Some(Modifiers::ALT), Code::KeyH);
    let h_south = HotKey::new(Some(Modifiers::ALT), Code::KeyJ);
    let h_north = HotKey::new(Some(Modifiers::ALT), Code::KeyK);
    let h_east = HotKey::new(Some(Modifiers::ALT), Code::KeyL);
    
    let h_resize_mode = HotKey::new(Some(Modifiers::ALT), Code::KeyR);
    let h_scratchpad = HotKey::new(Some(Modifiers::ALT), Code::Minus);
    
    manager.register(h_bsp).unwrap();
    manager.register(h_monocle).unwrap();
    manager.register(h_west).unwrap();
    manager.register(h_south).unwrap();
    manager.register(h_north).unwrap();
    manager.register(h_east).unwrap();
    manager.register(h_resize_mode).unwrap();
    manager.register(h_scratchpad).unwrap();
    
    // -- Resize Mode Hotkeys (No modifiers) --
    let r_west = HotKey::new(None, Code::KeyH);
    let r_south = HotKey::new(None, Code::KeyJ);
    let r_north = HotKey::new(None, Code::KeyK);
    let r_east = HotKey::new(None, Code::KeyL);
    let r_esc = HotKey::new(None, Code::Escape);
    let r_enter = HotKey::new(None, Code::Enter);
    
    let receiver = GlobalHotKeyEvent::receiver();
    std::thread::spawn(move || {
        let mut current_mode = KeyMode::Main;
        
        while let Ok(event) = receiver.recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                
                // Mode transitions
                if current_mode == KeyMode::Main && event.id == h_resize_mode.id() {
                    log::info!("Entering RESIZE mode");
                    current_mode = KeyMode::Resize;
                    manager.register(r_west).unwrap();
                    manager.register(r_south).unwrap();
                    manager.register(r_north).unwrap();
                    manager.register(r_east).unwrap();
                    manager.register(r_esc).unwrap();
                    manager.register(r_enter).unwrap();
                    continue;
                }
                
                if current_mode == KeyMode::Resize && (event.id == r_esc.id() || event.id == r_enter.id()) {
                    log::info!("Entering MAIN mode");
                    current_mode = KeyMode::Main;
                    manager.unregister(r_west).unwrap();
                    manager.unregister(r_south).unwrap();
                    manager.unregister(r_north).unwrap();
                    manager.unregister(r_east).unwrap();
                    manager.unregister(r_esc).unwrap();
                    manager.unregister(r_enter).unwrap();
                    continue;
                }

                let mut s = state.lock().unwrap();
                let active = s.skylight.get_active_space(s.connection_id);
                
                // Action processing based on mode
                match current_mode {
                    KeyMode::Main => {
                        if event.id == h_bsp.id() {
                            if let Some(engine) = s.layouts.get_mut(&active) {
                                engine.set_mode(LayoutMode::Bsp);
                                s.retile_space(active);
                            }
                        } else if event.id == h_monocle.id() {
                            if let Some(engine) = s.layouts.get_mut(&active) {
                                engine.set_mode(LayoutMode::Monocle);
                                s.retile_space(active);
                            }
                        } else if event.id == h_west.id() {
                            log::info!("Focus West");
                        } else if event.id == h_south.id() {
                            log::info!("Focus South");
                        } else if event.id == h_north.id() {
                            log::info!("Focus North");
                        } else if event.id == h_east.id() {
                            log::info!("Focus East");
                        } else if event.id == h_scratchpad.id() {
                            s.toggle_scratchpad(active);
                        }
                    }
                    KeyMode::Resize => {
                        if event.id == r_west.id() {
                            log::info!("Resize West -50");
                        } else if event.id == r_south.id() {
                            log::info!("Resize South +50");
                        } else if event.id == r_north.id() {
                            log::info!("Resize North -50");
                        } else if event.id == r_east.id() {
                            log::info!("Resize East +50");
                        }
                    }
                }
            }
        }
    });
}
