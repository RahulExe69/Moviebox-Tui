//! JNI entry points used by the Android Capacitor plugin.
//! A fresh token is read from the device CSPRNG; neither the token nor request contents are logged.

use crate::mobile_api::MobileApi;
use crate::mobile_server::{MobileServerHandle, start_loopback};
use std::ffi::{CString, c_char, c_void};
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};
use tokio::runtime::{Builder, Runtime};

type RawJniEnv = *mut *const c_void;
type JClass = *mut c_void;
type JString = *mut c_void;

struct RunningBackend {
    server: MobileServerHandle,
    _runtime: Runtime,
    token: String,
    port: u16,
}
impl RunningBackend {
    fn payload(&self) -> String {
        format!(
            r#"{{"baseUrl":"http://127.0.0.1:{}","token":"{}"}}"#,
            self.port, self.token
        )
    }
}

static BACKEND: OnceLock<Mutex<Option<RunningBackend>>> = OnceLock::new();
fn backend_state() -> &'static Mutex<Option<RunningBackend>> {
    BACKEND.get_or_init(|| Mutex::new(None))
}

fn fresh_token() -> Result<String, &'static str> {
    let mut entropy = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut entropy))
        .map_err(|_| "Secure random source unavailable")?;
    let mut token = String::with_capacity(64);
    for byte in entropy {
        use std::fmt::Write as _;
        let _ = write!(token, "{byte:02x}");
    }
    Ok(token)
}

fn start_backend_payload() -> Result<String, &'static str> {
    let mut state = backend_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(running) = state.as_ref() {
        return Ok(running.payload());
    }
    let token = fresh_token()?;
    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "Could not start the local runtime")?;
    let address = SocketAddr::from(([127, 0, 0, 1], 0));
    let server = runtime
        .block_on(start_loopback(address, MobileApi::new(), token.clone()))
        .map_err(|_| "Could not bind the local API")?;
    let port = server.local_addr().port();
    let running = RunningBackend {
        server,
        _runtime: runtime,
        token,
        port,
    };
    let payload = running.payload();
    *state = Some(running);
    Ok(payload)
}

fn stop_backend() {
    let mut state = backend_state().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(running) = state.take() {
        running.server.stop();
        drop(running);
    }
}

/// NewStringUTF is entry 167 in the standard JNI native interface table.
unsafe fn java_string(env: RawJniEnv, value: &str) -> JString {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    let clean = value.replace('\0', " ");
    let c_value = match CString::new(clean) {
        Ok(value) => value,
        Err(_) => CString::new("Local backend error").expect("static text has no NUL"),
    };
    let functions = unsafe { *env as *const *const c_void };
    if functions.is_null() {
        return std::ptr::null_mut();
    }
    let function_pointer = unsafe { *functions.add(167) };
    if function_pointer.is_null() {
        return std::ptr::null_mut();
    }
    let new_string_utf: unsafe extern "system" fn(RawJniEnv, *const c_char) -> JString =
        unsafe { std::mem::transmute(function_pointer) };
    unsafe { new_string_utf(env, c_value.as_ptr()) }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_sevenx_cinema_MobileBackendPlugin_nativeStartServer(
    env: RawJniEnv,
    _class: JClass,
) -> JString {
    let payload = match start_backend_payload() {
        Ok(payload) => payload,
        Err(message) => format!(r#"{{"error":"{}"}}"#, message),
    };
    unsafe { java_string(env, &payload) }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_sevenx_cinema_MobileBackendPlugin_nativeStopServer(
    _env: RawJniEnv,
    _class: JClass,
) {
    stop_backend();
}
