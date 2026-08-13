#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod core;
mod ui;
mod utils;

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("💻 老五系统安装助手")
            .with_inner_size([700.0, 580.0])
            .with_resizable(false)
            .with_minimize_button(true)
            .with_close_button(false),
        ..Default::default()
    };

    eframe::run_native(
        "老五系统安装助手",
        options,
        Box::new(|_cc| Ok(Box::new(LaoWuApp::new()))),
    )
}

struct LaoWuApp {
    sys_path: String,
    target_drive: String,
    logs: Arc<Mutex<Vec<String>>>,
    running: bool,
}

impl LaoWuApp {
    fn new() -> Self {
        Self {
            sys_path: "".to_string(),
            target_drive: "C:".to_string(),
            logs: Arc::new(Mutex::new(vec!["✅ 老五安装器已就绪".to_string()])),
            running: false,
        }
    }

    fn add_log(&self, msg: &str) {
        println!("{}", msg);
        let mut v = self.logs.lock().unwrap();
        v.push(msg.to_string());
        if v.len() > 100 { v.remove(0); }
    }

    fn start_install(&mut self) {
        let path = self.sys_path.clone();
        let drive = self.target_drive.clone();
        let logs = Arc::clone(&self.logs);
        
        self.running = true;
        self.add_log(">>> 正在启动系统安装流程...");

        thread::spawn(move || {
            if !std::path::Path::new(&path).exists() {
                logs.lock().unwrap().push("❌ 错误: 找不到镜像文件！".into());
                return;
            }
            logs.lock().unwrap().push(format!(">>> 镜像已锁定: {}", path));

            logs.lock().unwrap().push(format!(">>> 正在格式化 {} 盘...", drive));
            if let Err(e) = core::disk::DiskManager::format_partition(&drive) {
                logs.lock().unwrap().push(format!("❌ 格式化失败: {}", e));
                return;
            }
            logs.lock().unwrap().push("✅ 格式化完成".into());

            logs.lock().unwrap().push(">>> 正在释放系统镜像...".into());
            let success = if path.ends_with(".gho") {
                let ghost = core::ghost::Ghost::new();
                if ghost.is_available() {
                    let parts = core::disk::DiskManager::get_partitions().unwrap_or_default();
                    ghost.restore_image_to_letter(&path, &format!("{}:\\", drive), &parts, None).is_ok()
                } else {
                    logs.lock().unwrap().push("⚠️ 警告: 未检测到 GHOST 工具".into());
                    false
                }
            } else {
                let dism = core::dism::Dism::new();
                dism.apply_image(&path, &format!("{}:\\", drive), 0, None).is_ok()
            };

            if !success {
                logs.lock().unwrap().push("❌ 系统镜像释放失败".into());
                return;
            }
            logs.lock().unwrap().push("✅ 系统镜像释放完成".into());

            logs.lock().unwrap().push(">>> 正在修复引导配置...".into());
            let boot = core::bcdedit::BootManager::new();
            let uefi = core::disk::DiskManager::detect_uefi_mode();
            if let Err(e) = boot.repair_boot_advanced(&format!("{}:\\", drive), uefi) {
                logs.lock().unwrap().push(format!("⚠️ 引导配置修复警告: {}", e));
            } else {
                logs.lock().unwrap().push("✅ 引导修复完成".into());
            }

            logs.lock().unwrap().push("================================".into());
            logs.lock().unwrap().push("🎉 系统镜像安装流程完成！".into());
        });
    }
}

impl eframe::App for LaoWuApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("💻 老五系统一键安装器");
            ui.separator();

            ui.label("1. 请选择系统镜像 (.gho / .iso / .wim)");
            ui.horizontal(|ui| {
                let width = ui.available_width() - 90.0;
                ui.add(egui::TextEdit::singleline(&mut self.sys_path).desired_width(width));
                if ui.button("浏览文件").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_file() {
                        self.sys_path = p.to_string_lossy().to_string();
                        self.add_log(&format!("已选择: {}", self.sys_path));
                    }
                }
            });

            ui.separator();
            ui.label("2. 选择安装目标盘符 (例如 C:)");
            ui.text_edit_singleline(&mut self.target_drive);

            ui.add_space(20.0);
            
            let btn_text = if self.running { "🔄 正在执行中..." } else { "🚀 立即一键重装" };
            let btn_color = if self.running { egui::Color32::GRAY } else { egui::Color32::GREEN };

            ui.centered_and_justified(|ui| {
                if ui.add_sized(
                    ui.available_size_before_wrap(),
                    egui::Button::new(btn_text).fill(btn_color),
                ).clicked() && !self.running {
                    if self.sys_path.is_empty() {
                        self.add_log("⚠️ 请先选择系统镜像文件！");
                    } else {
                        self.start_install(); // 真正调用
                    }
                }
            });
  
            ui.add_space(20.0);
            ui.separator();
            ui.label("📝 运行日志");
            let logs = self.logs.lock().unwrap();
            for line in logs.iter() {
                ui.monospace(line);
            }
        });
        
        if self.running {
            ctx.request_repaint();
        }
    }
}
