#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod core;
mod ui;
mod utils;

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("💻 老五系统安装工具")
            .with_inner_size([600.0, 500.0])
            .with_resizable(false)
            .with_minimize_button(true)
            .with_close_button(false),
        ..Default::default()
    };

    eframe::run_native(
        "老五系统安装工具",
        options,
        Box::new(|_cc| Ok(Box::new(LaoWuApp::new()))),
    )
}

struct LaoWuApp {
    sys_path: String,       // 镜像路径
    target_drive: String,   // 目标磁盘 (user can enter "C" or "C:")
    logs: Arc<Mutex<Vec<String>>>,
    running: Arc<AtomicBool>, // allow background thread to clear this
}

impl LaoWuApp {
    fn new() -> Self {
        Self {
            sys_path: "".to_string(),
            target_drive: "C:".to_string(),
            logs: Arc::new(Mutex::new(vec!["✅ 老五安装器已就绪！".to_string()])),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn add_log(&self, msg: &str) {
        println!("{}", msg);
        if let Ok(mut v) = self.logs.lock() {
            v.push(msg.to_string());
            if v.len() > 100 { v.remove(0); }
        }
    }

    fn start_install(&mut self) {
        // mark running
        self.running.store(true, Ordering::SeqCst);
        self.add_log(">>> 正在启动无依赖安装流程...");

        let path = self.sys_path.clone();
        let drive = self.target_drive.clone();
        let logs = Arc::clone(&self.logs);
        let running_flag = Arc::clone(&self.running);

        thread::spawn(move || {
            // ensure running is cleared at the end
            struct ClearOnDrop(Arc<AtomicBool>);
            impl Drop for ClearOnDrop {
                fn drop(&mut self) { self.0.store(false, Ordering::SeqCst); }
            }
            let _guard = ClearOnDrop(running_flag);

            if !std::path::Path::new(&path).exists() {
                if let Ok(mut v) = logs.lock() {
                    v.push("❌ 镜像文件不存在，请确认选择!".into());
                }
                return;
            }

            if let Ok(mut v) = logs.lock() {
                v.push(format!(">>> 镜像已锁定: {}", path));
                v.push(format!(">>> 正在格式化 {} 盘...", drive));
            }

            // normalize drive forms: letter ("C"), with_colon ("C:"), root ("C:\")
            let drive_letter = drive.trim_end_matches(':').to_string();
            let drive_with_colon = format!("{}:", drive_letter);
            let drive_root = format!("{}:\\", drive_letter);

            if let Err(e) = core::disk::DiskManager::format_partition(&drive_with_colon) {
                if let Ok(mut v) = logs.lock() {
                    v.push(format!("❌ 格式化失败: {}", e));
                }
                return;
            }

            if let Ok(mut v) = logs.lock() {
                v.push("✅ 格式化完成".into());
                v.push(">>> 正在释放镜像...".into());
            }

            let success = if path.ends_with(".gho") {
                let ghost = core::ghost::Ghost::new();
                if ghost.is_available() {
                    let parts = core::disk::DiskManager::get_partitions().unwrap_or_default();
                    ghost.restore_image_to_letter(&path, &drive_root, &parts, None).is_ok()
                } else {
                    if let Ok(mut v) = logs.lock() {
                        v.push("⚠️ 警告: 未检测到 GHOST 工具，无法安装 GHO 格式镜像".into());
                    }
                    false
                }
            } else {
                let dism = core::dism::Dism::new();
                dism.apply_image(&path, &drive_root, 0, None).is_ok()
            };

            if !success {
                if let Ok(mut v) = logs.lock() {
                    v.push("❌ 系统镜像释放失败，请检查镜像文件!".into());
                }
                return;
            }

            if let Ok(mut v) = logs.lock() {
                v.push("✅ 镜像释放成功".into());
                v.push(">>> 正在修复引导配置...".into());
            }

            let boot = core::bcdedit::BootManager::new();
            let use_uefi = core::disk::DiskManager::detect_uefi_mode();

            if let Err(e) = boot.repair_boot_advanced(&drive_root, use_uefi) {
                if let Ok(mut v) = logs.lock() {
                    v.push(format!("⚠️ 引导配置修复警告: {}", e));
                }
            } else {
                if let Ok(mut v) = logs.lock() {
                    v.push("✅ 引导修复完成！".into());
                }
            }

            if let Ok(mut v) = logs.lock() {
                v.push("=================================".into());
                v.push("🎉 系统镜像重装完成，可以安全重启了！".into());
            }
        });
    }
}

impl eframe::App for LaoWuApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("💻 老五系统安装工具");
            ui.separator();

            ui.label("1. 请选择系统镜像 (.gho / .iso / .wim)");
            if ui.button("浏览文件").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    self.sys_path = p.to_string_lossy().to_string();
                    self.add_log(&format!("已选择镜像: {}", self.sys_path));
                }
            }
            ui.text_edit_singleline(&mut self.sys_path);

            ui.separator();
            ui.label("2. 选择安装目标磁盘（例如 C 或 C:）");
            ui.text_edit_singleline(&mut self.target_drive);

            ui.add_space(20.0);

            let is_running = self.running.load(Ordering::SeqCst);
            let btn_text = if is_running { "🔄 正在释放中..." } else { "🚀 立即开始重装" };
            let btn_color = if is_running { egui::Color32::GRAY } else { egui::Color32::GREEN };

            ui.centered_and_justified(|ui| {
                if ui.add_sized(
                    ui.available_size_before_wrap(),
                    egui::Button::new(btn_text).fill(btn_color),
                ).clicked() && !is_running {

                    if self.sys_path.is_empty() {
                        self.add_log("⚠️ 请先选好镜像文件！");
                    } else {
                        self.start_install(); // 一次正确调用（不需要参数）
                    }
                }
            });

            ui.add_space(20.0);
            ui.separator();
            ui.label("📝 运行日志展示");
            let logs = self.logs.lock().unwrap();
            for line in logs.iter() {
                ui.monospace(line);
            }
        });

        if self.running.load(Ordering::SeqCst) {
            ctx.request_repaint();
        }
    }
}
