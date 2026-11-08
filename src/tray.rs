use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use std::thread;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM, CloseHandle};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    TrackPopupMenu, TranslateMessage, CW_USEDEFAULT, HICON, HMENU, IDI_APPLICATION, MF_STRING,
    MSG, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_DESTROY,
    WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};

pub const TRAY_MENU_RESET: &str = "Reiniciar partida";
pub const TRAY_MENU_RELOAD: &str = "Recargar configuración";
pub const TRAY_MENU_EXIT: &str = "Salir";

pub const ID_TRAY_RESET: usize = 1001;
pub const ID_TRAY_RELOAD: usize = 1002;
pub const ID_TRAY_EXIT: usize = 1003;

const WM_TRAY_CALLBACK: u32 = WM_APP + 1;
const TRAY_ICON_ID: u32 = 101;

static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ResetMatch,
    ReloadConfig,
    ExitApp,
}

pub fn map_menu_command(cmd_id: usize) -> Option<TrayAction> {
    match cmd_id {
        ID_TRAY_RESET => Some(TrayAction::ResetMatch),
        ID_TRAY_RELOAD => Some(TrayAction::ReloadConfig),
        ID_TRAY_EXIT => Some(TrayAction::ExitApp),
        _ => None,
    }
}

pub struct SingleInstanceGuard(pub(crate) HANDLE);

unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() && !self.0 .0.is_null() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub fn acquire_single_instance_lock(mutex_name: &str) -> Option<SingleInstanceGuard> {
    let wide: Vec<u16> = mutex_name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateMutexW(None, BOOL(0), PCWSTR(wide.as_ptr())).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(SingleInstanceGuard(handle))
    }
}

pub fn remove_tray_icon() {
    let hwnd_raw = TRAY_HWND.swap(0, Ordering::Relaxed);
    if hwnd_raw != 0 {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_CALLBACK => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP {
                if let Some(action) = show_tray_popup_menu(hwnd) {
                    if let Some(tx) = TRAY_TX.get() {
                        let _ = tx.send(action);
                    }
                    if action == TrayAction::ExitApp {
                        remove_tray_icon();
                        std::process::exit(0);
                    }
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray_icon();
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

static TRAY_TX: OnceLock<Sender<TrayAction>> = OnceLock::new();

unsafe fn show_tray_popup_menu(hwnd: HWND) -> Option<TrayAction> {
    let hmenu = CreatePopupMenu().ok()?;
    let _ = AppendMenuW(
        hmenu,
        MF_STRING,
        ID_TRAY_RESET,
        w!("Reiniciar partida"),
    );
    let _ = AppendMenuW(
        hmenu,
        MF_STRING,
        ID_TRAY_RELOAD,
        w!("Recargar configuración"),
    );
    let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, w!("Salir"));

    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);

    let _ = SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        hmenu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    let _ = DestroyMenu(hmenu);

    if cmd.0 > 0 {
        map_menu_command(cmd.0 as usize)
    } else {
        None
    }
}

pub fn spawn_system_tray(tx: Sender<TrayAction>) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("system_tray_manager".into())
        .spawn(move || {
            let _ = TRAY_TX.set(tx);
            unsafe {
                let class_name = w!("DeadlockOverlayTrayClass");
                let icon: HICON = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

                let wc = WNDCLASSW {
                    lpfnWndProc: Some(tray_window_proc),
                    lpszClassName: class_name,
                    hIcon: icon,
                    ..Default::default()
                };
                let _ = RegisterClassW(&wc);

                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    class_name,
                    w!("DeadlockOverlayTrayWindow"),
                    WS_OVERLAPPED,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    None,
                    HMENU(std::ptr::null_mut()),
                    None,
                    None,
                )
                .unwrap_or(HWND(std::ptr::null_mut()));

                if hwnd.0.is_null() {
                    return;
                }

                TRAY_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

                // Add icon to system tray
                let mut tooltip = [0u16; 128];
                let tip_text = "Deadlock Macro Overlay";
                for (i, c) in tip_text.encode_utf16().enumerate().take(127) {
                    tooltip[i] = c;
                }

                let nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: TRAY_ICON_ID,
                    uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                    uCallbackMessage: WM_TRAY_CALLBACK,
                    hIcon: icon,
                    szTip: tooltip,
                    ..Default::default()
                };

                let _ = Shell_NotifyIconW(NIM_ADD, &nid);

                // Standard Win32 message loop
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                remove_tray_icon();
            }
        })
        .expect("Failed to spawn system tray thread")
}
