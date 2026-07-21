with open('src/main.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Find tray module start and end
tray_start = content.find('#[cfg(windows)]\nmod tray {')
tray_end = content.find('/// 非 Windows 平台的空托盘实现', tray_start)

new_tray = '''#[cfg(windows)]
mod tray {
    use std::time::Duration;
    use tray_icon::menu::*;
    use tray_icon::{Icon, TrayIconEvent};
    use log::info;

    mod win32 {
        #[repr(C)] pub struct POINT { pub x: i32, pub y: i32 }
        #[repr(C)] pub struct MSG { pub hwnd: isize, pub message: u32, pub wparam: usize, pub lparam: isize, pub time: u32, pub pt: POINT }
        pub const PM_REMOVE: u32 = 0x0001;
        pub const WM_QUIT: u32 = 0x0012;
        extern "system" {
            pub fn PeekMessageW(msg: *mut MSG, hwnd: isize, wMin: u32, wMax: u32, remove: u32) -> i32;
            pub fn TranslateMessage(msg: *const MSG) -> i32;
            pub fn DispatchMessageW(msg: *const MSG) -> isize;
        }
    }

    fn create_icon() -> Icon {
        let size = 32u32;
        let half = size as f32 / 2.0;
        let radius = half - 1.2;
        let mut rgba = vec![0u8; (size*size*4) as usize];
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (x as f32 - half, y as f32 - half);
                let i = ((y*size+x)*4) as usize;
                if (dx*dx+dy*dy).sqrt() <= rgba[i+3] { rgba[i..i+4].copy_from_slice(if in_m { &[255,255,255,255] } else { &[255,209,0,255] }); }
            }
        }
    }

    fn open_browser(port: &str) {
        let _ = std::process::Command::new("cmd").args(["/c","start","",&format!("http://localhost:{}",port)]).spawn();
    }

    fn build_menu() -> Menu {
        let menu = Menu::new();
        let open = MenuItemBuilder::new().id(MenuId::new("open")).text("打开网页").enabled(true).build();
        let refresh = MenuItemBuilder::new().id(MenuId::new("refresh")).text("同步数据").enabled(true).build();
        let sep = PredefinedMenuItem::separator();
        let settings = MenuItemBuilder::new().id(MenuId::new("settings")).text("设置").enabled(true).build();
        let about = MenuItemBuilder::new().id(MenuId::new("about")).text("关于").enabled(true).build();
        let exit = MenuItemBuilder::new().id(MenuId::new("exit")).text("退出").enabled(true).build();
        let _ = menu.append_items(&[&open, &refresh, &sep, &settings, &about, &exit]);
        menu
    }

    fn handle_menu(event: &MenuEvent, exe_dir: &str, port: &str) {
        match event.id().as_ref() {
            "open" => open_browser(port),
            "refresh" => {
                let s = crate::rust_refresh(exe_dir, true);
                info!("同步: +{} 新, {} 更", s.new, s.updated);
            }
            "settings" => open_browser(port),
            "about" => info!("美团订单管理系统 v{}", env!("CARGO_PKG_VERSION")),
            "exit" => { info!("退出"); std::process::exit(0); }
            _ => {}
        }
    }

    pub fn run(exe_dir: String, _html_dir: String, port: String) {
        let _tray = tray_icon::TrayIconBuilder::new()
            .with_tooltip("美团订单管理 - 点击打开网页")
            .with_icon(create_icon())
            .with_menu(Box::new(build_menu()))
            .build();
        info!("系统托盘已创建");

        std::thread::sleep(Duration::from_secs(1));
        unsafe {
            let mut msg: win32::MSG = std::mem::zeroed();
            loop {
                while win32::PeekMessageW(&mut msg, 0, 0, 0, win32::PM_REMOVE) != 0 {
                    if msg.message == win32::WM_QUIT { return; }
                    win32::TranslateMessage(&msg); win32::DispatchMessageW(&msg);
                }
                while let Ok(event) = MenuEvent::receiver().try_recv() { handle_menu(&event, &exe_dir, &port); }
                while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    if matches!(event, TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. }) {
                        open_browser(&port);
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

'''

content = content[:tray_start] + new_tray + content[tray_end:]

with open('src/main.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("OK")
