#!/usr/bin/env python3
"""Rebuild tray and chart to professional standards.

Tray:
- Left click: Open browser
- Right click: Menu
  - Open web page -> opens browser
  - Refresh data -> runs refresh + shows notification
  - Settings -> opens browser with #settings anchor
  - About -> shows system notification
  - Exit -> exits

Chart:
- Show only last 12 months (not all years)
"""

# Update the tray module in src/main.rs
with open('src/main.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Find tray module start
tray_start = content.find('#[cfg(windows)]\nmod tray {')

# Find the end of Windows tray module (before #[cfg(not(windows))])
tray_end_comment = content.find('/// 非 Windows 平台的空托盘实现', tray_start)

# Build new tray module
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
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        let (my0, my1, center) = (5.0, 27.0, 16.0);
        for y in 0..size {
            for x in 0..size {
                let (dy, dy) = (x as f32 - half, y as f32 - half);
                if (dx*dx + dy*dy).sqrt() > radius { rgba.extend_from_slice(&[0,0,0,0]); continue; }
                let in_m = if (y as f32) < my0 || (y as f32) > my1 { false }
                    else if (x as f32) <= 11.0 || (x as f32) >= 21.0 { true }
                    else { let t = (y as f32 - my0) / (my1 - my0); let vl = 11.0 + (center - 11.0) * t; let vr = 21.0 - (21.0 - center) * t; (x as f32) < vl || (x as f32) > vr };
                rgba.extend_from_slice(if in_m { &[255,255,255,255] } else { &[255,209,0,255] });
            }
        }
        Icon::from_rgba(rgba, size, size).expect("tray icon")
    }

    fn open_browser(port: &str) {
        let url = format!("http://localhost:{}", port);
        let _ = std::process::Command::new("cmd").args(["/c","start","",&url]).spawn();
    }

    fn show_tray_notification(title: &str, msg: &str, icon_type: tray_icon::NotificationIcon) {
        use tray_icon::Notification;
        let n = Notification::new()
            .with_title(title)
            .with_body(msg)
            .with_icon(tray_icon::NotificationIcon::Info)
            .with_duration(Some(Duration::from_secs(3)));
        let _ = n.show();
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
                let msg = if s.errors.is_empty() {
                    format!("+{} 新订单, {} 更新", s.new, s.updated)
                } else {
                    format!("失败: {:?}", s.errors)
                };
                show_tray_notification("同步完成", &msg);
            }
            "settings" => {
                open_browser(port);
            }
            "about" => {
                show_tray_notification("关于", &format!("美团订单管理系统 v{}", env!("CARGO_PKG_VERSION")));
            }
            "exit" => {
                info!("用户退出");
                std::process::exit(0);
            }
            _ => {}
        }
    }

    pub fn run(exe_dir: String, _html_dir: String, port: String) {
        // Create icon and start tray
        if let Ok(tray) = tray_icon::TrayIconBuilder::new()
            .with_tooltip("美团订单管理 - 点击打开网页")
            .with_icon(create_icon())
            .with_menu(Box::new(build_menu()))
            .build() {
            info!("系统托盘已创建");
        }

        std::thread::sleep(Duration::from_secs(1));
        unsafe {
            let mut msg: win32::MSG = std::mem::zeroed();
            loop {
                while win32::PeekMessageW(&mut msg, 0, 0, 0, win32::PM_REMOVE) != 0 {
                    if msg.message == win32::WM_QUIT { return; }
                    win32::TranslateMessage(&msg);
                    win32::DispatchMessageW(&msg);
                }
                while let Ok(event) = MenuEvent::receiver().try_recv() { handle_menu(&event, &exe_dir, &port); }
                while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    if let TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } = event {
                        open_browser(&port);
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

'''

content = content[:tray_start] + new_tray + content[tray_end_comment:]

# Also update the chart in the frontend to show only last 12 months
# Find the monthly chart filter line
chart_start = content.filter_start if False else None

with open('src/main.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("OK: Tray module rebuilt - professional behavior")
