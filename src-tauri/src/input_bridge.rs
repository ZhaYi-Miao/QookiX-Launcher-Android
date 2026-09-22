use anyhow::Result;
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

pub type GlfwKeyCallback =
    unsafe extern "C" fn(window: *mut c_void, key: i32, scancode: i32, action: i32, mods: i32);
pub type GlfwMouseButtonCallback =
    unsafe extern "C" fn(window: *mut c_void, button: i32, action: i32, mods: i32);
pub type GlfwCursorPosCallback =
    unsafe extern "C" fn(window: *mut c_void, x: f64, y: f64);
pub type GlfwScrollCallback =
    unsafe extern "C" fn(window: *mut c_void, xoffset: f64, yoffset: f64);

#[derive(Clone, Copy)]
struct GlfwInputEvent {
    event_type: i32,
    i1: i32,
    i2: i32,
    i3: i32,
    i4: i32,
}

pub struct GlfwCallbackTable {
    on_key: Option<GlfwKeyCallback>,
    on_mouse_button: Option<GlfwMouseButtonCallback>,
    on_cursor_pos: Option<GlfwCursorPosCallback>,
    on_scroll: Option<GlfwScrollCallback>,
}

impl GlfwCallbackTable {
    fn new() -> Self {
        Self {
            on_key: None,
            on_mouse_button: None,
            on_cursor_pos: None,
            on_scroll: None,
        }
    }
}

#[derive(Debug)]
pub enum CallbackKind {
    Key,
    MouseButton,
    CursorPos,
    Scroll,
}

const RING_CAPACITY: usize = 128;

struct EventRingBuffer {
    events: std::cell::UnsafeCell<[GlfwInputEvent; RING_CAPACITY]>,
    in_index: AtomicUsize,
    out_index: AtomicUsize,
}

unsafe impl Send for EventRingBuffer {}
unsafe impl Sync for EventRingBuffer {}

impl EventRingBuffer {
    fn new() -> Self {
        Self {
            events: std::cell::UnsafeCell::new([GlfwInputEvent {
                event_type: 0,
                i1: 0,
                i2: 0,
                i3: 0,
                i4: 0,
            }; RING_CAPACITY]),
            in_index: AtomicUsize::new(0),
            out_index: AtomicUsize::new(0),
        }
    }

    fn push(&self, event: GlfwInputEvent) {
        let in_idx = self.in_index.load(Ordering::Relaxed);
        let out_idx = self.out_index.load(Ordering::Acquire);
        if in_idx.wrapping_sub(out_idx) >= RING_CAPACITY {
            self.out_index.fetch_add(1, Ordering::Release);
            tracing::warn!("Input ring buffer full, dropping oldest event");
        }
        let slot = in_idx % RING_CAPACITY;
        unsafe {
            (*self.events.get())[slot] = event;
        }
        self.in_index.fetch_add(1, Ordering::Release);
    }

    fn pump(&self, callbacks: &GlfwCallbackTable, window: *mut c_void) {
        let in_idx = self.in_index.load(Ordering::Acquire);
        let mut out_idx = self.out_index.load(Ordering::Relaxed);
        while out_idx < in_idx {
            let slot = out_idx % RING_CAPACITY;
            let event = unsafe { (*self.events.get())[slot] };
            dispatch_event(&event, callbacks, window);
            out_idx += 1;
        }
        self.out_index.store(out_idx, Ordering::Release);
    }

    fn pending(&self) -> usize {
        let in_idx = self.in_index.load(Ordering::Acquire);
        let out_idx = self.out_index.load(Ordering::Acquire);
        in_idx.wrapping_sub(out_idx)
    }
}

fn dispatch_event(event: &GlfwInputEvent, callbacks: &GlfwCallbackTable, window: *mut c_void) {
    match event.event_type {
        1005 => {
            if let Some(cb) = callbacks.on_key {
                unsafe { cb(window, event.i1, event.i2, event.i3, event.i4) };
            }
        }
        1006 => {
            if let Some(cb) = callbacks.on_mouse_button {
                unsafe { cb(window, event.i1, event.i2, event.i3) };
            }
        }
        1003 => {
            if let Some(cb) = callbacks.on_cursor_pos {
                let x = event.i1 as f64 / 1000.0;
                let y = event.i2 as f64 / 1000.0;
                unsafe { cb(window, x, y) };
            }
        }
        1007 => {
            if let Some(cb) = callbacks.on_scroll {
                let x = event.i1 as f64 / 1000.0;
                let y = event.i2 as f64 / 1000.0;
                unsafe { cb(window, x, y) };
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key {
        key_code: i32,
        scancode: i32,
        action: i32,
        mods: i32,
    },
    MouseButton {
        button: i32,
        action: i32,
        mods: i32,
    },
    CursorPos {
        x: f64,
        y: f64,
    },
    Scroll {
        x: f64,
        y: f64,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("INPUT_NOT_READY")]
    InputNotReady,
    #[error("INVALID_EVENT: {0}")]
    InvalidEvent(String),
}

pub struct InputBridge {
    ready: bool,
    callback_table: GlfwCallbackTable,
    ring_buffer: EventRingBuffer,
    window_handle: *mut c_void,
}

unsafe impl Send for InputBridge {}

impl InputBridge {
    pub fn new() -> Self {
        Self {
            ready: false,
            callback_table: GlfwCallbackTable::new(),
            ring_buffer: EventRingBuffer::new(),
            window_handle: std::ptr::null_mut(),
        }
    }

    pub fn send_event(&mut self, event: InputEvent) -> Result<()> {
        if !self.ready {
            tracing::warn!("Input event dropped (not ready): {:?}", event);
            return Ok(());
        }

        let glfw_event = match event {
            InputEvent::Key {
                key_code,
                scancode,
                action,
                mods,
            } => GlfwInputEvent {
                event_type: 1005,
                i1: key_code,
                i2: scancode,
                i3: action,
                i4: mods,
            },
            InputEvent::MouseButton {
                button,
                action,
                mods,
            } => GlfwInputEvent {
                event_type: 1006,
                i1: button,
                i2: action,
                i3: mods,
                i4: 0,
            },
            InputEvent::CursorPos { x, y } => GlfwInputEvent {
                event_type: 1003,
                i1: (x * 1000.0) as i32,
                i2: (y * 1000.0) as i32,
                i3: 0,
                i4: 0,
            },
            InputEvent::Scroll { x, y } => GlfwInputEvent {
                event_type: 1007,
                i1: (x * 1000.0) as i32,
                i2: (y * 1000.0) as i32,
                i3: 0,
                i4: 0,
            },
        };

        self.ring_buffer.push(glfw_event);
        Ok(())
    }

    pub fn register_callback(&mut self, kind: CallbackKind, ptr: *mut c_void) {
        if ptr.is_null() {
            return;
        }
        match kind {
            CallbackKind::Key => {
                self.callback_table.on_key = Some(unsafe { std::mem::transmute::<*mut c_void, GlfwKeyCallback>(ptr) });
            }
            CallbackKind::MouseButton => {
                self.callback_table.on_mouse_button = Some(unsafe { std::mem::transmute::<*mut c_void, GlfwMouseButtonCallback>(ptr) });
            }
            CallbackKind::CursorPos => {
                self.callback_table.on_cursor_pos = Some(unsafe { std::mem::transmute::<*mut c_void, GlfwCursorPosCallback>(ptr) });
            }
            CallbackKind::Scroll => {
                self.callback_table.on_scroll = Some(unsafe { std::mem::transmute::<*mut c_void, GlfwScrollCallback>(ptr) });
            }
        }
        tracing::info!("Callback registered: {:?}", kind);
    }

    pub fn pump_events(&mut self) {
        let window = self.window_handle;
        let callbacks = &self.callback_table;
        self.ring_buffer.pump(callbacks, window);
    }

    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;
        tracing::info!("Input bridge ready: {}", ready);
    }

    pub fn set_window_handle(&mut self, window: *mut c_void) {
        self.window_handle = window;
    }

    pub fn pending_events(&self) -> usize {
        self.ring_buffer.pending()
    }
}

static INPUT_BRIDGE: OnceLock<Mutex<InputBridge>> = OnceLock::new();

pub fn init() {
    INPUT_BRIDGE.get_or_init(|| Mutex::new(InputBridge::new()));
}

pub fn send_event(event: InputEvent) -> Result<()> {
    if let Some(bridge) = INPUT_BRIDGE.get() {
        if let Ok(mut bridge) = bridge.lock() {
            return bridge.send_event(event);
        }
    }
    Ok(())
}

pub fn set_ready(ready: bool) {
    if let Some(bridge) = INPUT_BRIDGE.get() {
        if let Ok(mut bridge) = bridge.lock() {
            bridge.set_ready(ready);
        }
    }
}

pub fn set_window_handle(window: *mut c_void) {
    if let Some(bridge) = INPUT_BRIDGE.get() {
        if let Ok(mut bridge) = bridge.lock() {
            bridge.set_window_handle(window);
        }
    }
}

pub fn register_callback(kind: CallbackKind, ptr: *mut c_void) {
    if let Some(bridge) = INPUT_BRIDGE.get() {
        if let Ok(mut bridge) = bridge.lock() {
            bridge.register_callback(kind, ptr);
        }
    }
}

pub fn pump_events() {
    if let Some(bridge) = INPUT_BRIDGE.get() {
        if let Ok(mut bridge) = bridge.lock() {
            bridge.pump_events();
        }
    }
}

pub fn pending_events() -> usize {
    if let Some(bridge) = INPUT_BRIDGE.get() {
        if let Ok(bridge) = bridge.lock() {
            return bridge.pending_events();
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    static mut KEY_HITS: Vec<(i32, i32)> = Vec::new();
    static mut CURSOR_HITS: Vec<(f64, f64)> = Vec::new();

    unsafe extern "C" fn test_key_cb(_w: *mut c_void, key: i32, sc: i32, action: i32, mods: i32) {
        unsafe {
            KEY_HITS.push((key + sc, action + mods));
        }
    }

    unsafe extern "C" fn test_cursor_cb(_w: *mut c_void, x: f64, y: f64) {
        unsafe {
            CURSOR_HITS.push((x, y));
        }
    }

    #[test]
    fn test_ring_buffer_push_and_pump_key() {
        let buffer = EventRingBuffer::new();
        let mut table = GlfwCallbackTable::new();
        table.on_key = Some(test_key_cb);

        buffer.push(GlfwInputEvent {
            event_type: 1005,
            i1: 87,
            i2: 26,
            i3: 1,
            i4: 0,
        });
        assert_eq!(buffer.pending(), 1);

        let window: *mut c_void = std::ptr::null_mut();
        buffer.pump(&table, window);

        assert_eq!(buffer.pending(), 0);
        unsafe {
            assert_eq!(KEY_HITS.len(), 1);
            assert_eq!(KEY_HITS[0], (113, 1));
        }
    }

    #[test]
    fn test_ring_buffer_cursor_pos_float_scaling() {
        let buffer = EventRingBuffer::new();
        let mut table = GlfwCallbackTable::new();
        table.on_cursor_pos = Some(test_cursor_cb);

        buffer.push(GlfwInputEvent {
            event_type: 1003,
            i1: 12345,
            i2: -67890,
            i3: 0,
            i4: 0,
        });

        let window: *mut c_void = std::ptr::null_mut();
        buffer.pump(&table, window);

        unsafe {
            assert!((CURSOR_HITS[0].0 - 12.345).abs() < 1e-9);
            assert!((CURSOR_HITS[0].1 - (-67.89)).abs() < 1e-9);
        }
    }

    #[test]
    fn test_ring_buffer_unregistered_callback_skipped() {
        let buffer = EventRingBuffer::new();
        let table = GlfwCallbackTable::new();

        buffer.push(GlfwInputEvent {
            event_type: 1006,
            i1: 0,
            i2: 1,
            i3: 0,
            i4: 0,
        });

        let window: *mut c_void = std::ptr::null_mut();
        buffer.pump(&table, window);

        assert_eq!(buffer.pending(), 0);
    }

    #[test]
    fn test_ring_buffer_overflow_drops_oldest() {
        let buffer = EventRingBuffer::new();
        let table = GlfwCallbackTable::new();

        for i in 0..(RING_CAPACITY + 10) {
            buffer.push(GlfwInputEvent {
                event_type: 1005,
                i1: i as i32,
                i2: 0,
                i3: 1,
                i4: 0,
            });
        }

        assert!(buffer.pending() <= RING_CAPACITY);
    }

    #[test]
    fn test_send_event_not_ready_drops() {
        let mut bridge = InputBridge::new();
        bridge.set_ready(false);

        let result = bridge.send_event(InputEvent::Key {
            key_code: 87,
            scancode: 0,
            action: 1,
            mods: 0,
        });

        assert!(result.is_ok());
        assert_eq!(bridge.pending_events(), 0);
    }

    #[test]
    fn test_send_event_ready_writes_buffer() {
        let mut bridge = InputBridge::new();
        bridge.set_ready(true);

        bridge
            .send_event(InputEvent::Key {
                key_code: 87,
                scancode: 26,
                action: 1,
                mods: 0,
            })
            .unwrap();
        bridge
            .send_event(InputEvent::MouseButton {
                button: 0,
                action: 1,
                mods: 0,
            })
            .unwrap();
        bridge
            .send_event(InputEvent::CursorPos { x: 1.5, y: 2.5 })
            .unwrap();
        bridge
            .send_event(InputEvent::Scroll { x: 0.0, y: 3.0 })
            .unwrap();

        assert_eq!(bridge.pending_events(), 4);
    }

    #[test]
    fn test_event_type_encoding() {
        let mut bridge = InputBridge::new();
        bridge.set_ready(true);

        bridge
            .send_event(InputEvent::Key {
                key_code: 1,
                scancode: 0,
                action: 0,
                mods: 0,
            })
            .unwrap();
        bridge
            .send_event(InputEvent::MouseButton {
                button: 1,
                action: 0,
                mods: 0,
            })
            .unwrap();
        bridge.send_event(InputEvent::CursorPos { x: 0.0, y: 0.0 }).unwrap();
        bridge.send_event(InputEvent::Scroll { x: 0.0, y: 0.0 }).unwrap();

        let window: *mut c_void = std::ptr::null_mut();
        let table = GlfwCallbackTable::new();
        assert_eq!(bridge.pending_events(), 4);
        bridge.pump_events();
        let _ = window;
        let _ = table;
    }

    #[test]
    fn test_register_callback_null_ignored() {
        let mut bridge = InputBridge::new();
        bridge.register_callback(CallbackKind::Key, std::ptr::null_mut());
    }
}
