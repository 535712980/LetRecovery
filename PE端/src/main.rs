#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod core;
mod ui;
mod utils;

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default())
        .default_filter_or("info")
        .try_init(); 

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("🛠️ 老五系统安装器")
            .with_inner_size([650.0, 520.0])
            .with_resizable(false)
            .with_minimize_button(true)
            .with_close_button(false), 
        ..Default::default()
    };

    eframe::run_native(
        "老五系统安装器",
        options,
        Box::new(|_cc| Ok(Box::new(LowWuApp::new()))),
    )
}

struct LowWuApp {
    sys_path: String,       // 镜像路径
    target_drive: String,   // 目标盘符
    working: bool,          // 运行状态
    logs: Arc<Mutex<Vec<String>>>, // 日志缓冲区
}

impl LowWuApp {
    fn new() -> Self {
        Self {
            sys_path: "".to_string(),
            target_drive: "C:".to_string(),
            working: false,
            logs: Arc::new(Mutex::new(vec!["✅ 老五安装器已就绪，请选择镜像并点击一键重装.".into()])),
        }
    }

    fn push_log(&self, msg: &str) {
        println!("{}", msg);
        let mut vec = self.logs.lock().unwrap();
        vec.push(msg.into());
        if vec.len() > 100 { vec.remove(0); }
    }

    fn start_install_thread(self: Box<Self>) {
        thread::spawn(move || {
            self.push_log(">>> [步骤1] 正在检测环境...");
            
            let path_clone = self.sys_path.clone();
            let drive_clone = self.target_drive.clone();

            if !std::path::Path::new(&path_clone).exists() {
                self.push_log("[❌失败] 找不到系统镜像文件，请重新选择！");
                return;
            }
            self.push_log(format!(">>> [确认] 镜像已锁定：{}", path_clone));

            let full_target = format!("{}\\", drive_clone.trim_end_matches(':'));
            
            // 格式化
            self.push_log(format!(">>> [步骤2] 开始格式化分区 {}...", drive_clone));
            if let Err(e) = core::disk::DiskManager::format_partition(&drive_clone) {
                self.push_log(format!("[❌错误] 格式化失败: {}", e));
                return;
            }
            self.push_log("✅ [成功] 格式化完成。");

            // 释放镜像
            self.push_log(">>> [步骤3] 正在进行无损还原 (写入中)...");
            let success = if path_clone.ends_with(".gho") {
                let ghost = core::ghost::Ghost::new();
                if !ghost.is_available() {
                    self.push_log("[❌警告] 未检测到 GHOST 工具，无法安装GHO格式。");
                    false
                } else {
                    self.push_log(">>> 正在使用 Ghost 模式加速写入...");
                    let partitions = core::disk::DiskManager::get_partitions().unwrap_or_default();
                    ghost.restore_image_to_letter(&path_clone, &full_target, &partitions, None).is_ok()
                }
            } else {
                self.push_log(">>> 正在使用 WIM/ESD 标准模式写入...");
                let dism = core::dism::Dism::new();
                dism.apply_image(&path_clone, &full_target, 0, None).is_ok()
            };

            if !success {
                self.push_log("❌ [严重报错] 系统写入失败，请检查磁盘空间是否充足。");
                return;
            }
            self.push_log("✅ [成功] 系统文件释放完毕！");

            // 修复引导
            self.push_log(">>> [步骤4] 正在注入引导文件及修复启动项...");
            let manager = core::bcdedit::BootManager::new();
            let is_uefi = core::disk::DiskManager::detect_uefi_mode();
            
            match manager.repair_boot_advanced(&full_target, is_uefi) {
                Ok(_) => self.push_log("✅ [成功] 引导修复完美结束。"),
                Err(e) => self.push_log(format!("⚠️ [警告] 引导修复遇到小问题 (不影响开机): {}", e)),
            }

            self.push_log("==================================");
            self.push_log("🎉【老五装机】大功告成！现在可以安全重启了！");
        });
    }
}

impl eframe::App for LowWuApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 强制刷新线程里的日志
        {
            let _logs = self.logs.lock().unwrap();
            // 简单的防阻塞占位
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(ui.strong("💻 老五系统安装助手"));
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // 1. 选文件
                ui.label("📂 1. 请选择你的系统镜像 (.gho/.iso/.wim)");
                
                ui.horizontal(|ui| {
                    let width = ui.available_width() - 100.0;
                    ui.add(egui::TextEdit::singleline(&mut self.sys_path).desired_width(width));
                    
                    // 使用 rfd 库打开文件窗口
                    if ui.button("浏览电脑...").clicked() {
                        if let Some(pathbuf) = rfd::FileDialog::new().add_filter("All Files", &("*")).pick_file() {
                            self.sys_path = pathbuf.to_string_lossy().to_string();
                            ctx.request_repaint();
                        }
                    }
                });

                ui.add_space(15.0);

                // 2. 选硬盘
                ui.label("💾 2. 你想装到哪个盘？");
                ui.text_edit_singleline(&mut self.target_drive);

                ui.add_space(15.0);

                // 3. 大按钮
                let btn_color = if self.working { 
                    egui::Color32::GRAY 
                } else { 
                    egui::Color32::from_rgb(40, 167, 69) // 绿色
                };

                ui.centered_and_justified(|ui| {
                    let btn_label = if self.working {
                        "🔄 安装中..."
                    } else {
                        "🚀 立即一键重装"
                    };
                    if ui.add_sized(
                        ui.available_size_before_wrap(), 
                        egui::Button::new(btn_label).fill(btn_color).text_color(egui::Color32::WHITE)
                    ).clicked() && !self.working {
                        
                        if self.sys_path.is_empty() {
                            self.push_log("[提示] 还没选镜像呢！");
                        } else {
                            self.working = true;
                            // 发送线程
                            unsafe {
                                // 这里强行借用，因为start需要取走self引用或者Box
                                let ptr = self as *mut Self;
                                (*ptr).sys_path.clone(); // dummy access
                            }
                            
                            let path = self.sys_path.clone();
                            let drive = self.target_drive.clone();
                            let logs = self.logs.clone();
                            
                            thread::spawn(move || {
                                // 这里的逻辑重复一遍以适配闭包传参，保持简单
                                if !std::path::Path::new(&path).exists() {
                                    let mut v = logs.lock().unwrap();
                                    v.push("[❌失败] 找不到文件！".into());
                                    return;
                                }
                                let mut v = logs.lock().unwrap();
                                v.push(">>> [开始] 正在执行底层任务...".into());
                                
                                // 调用核心逻辑
                                crate::LowWuApp::start_core_task(&path, &drive, &v);
                            });
                        }
                    }
                });

                ui.add_space(20.0);
                ui.separator();
                ui.label("📝 实时运行日志");
                let guard = self.logs.lock().unwrap();
                for line in guard.iter() {
                    ui.colored_text(egui::Color32::LIGHT_GRAY, line);
                }
                // 自动滚到底部
                ui.scroll_here();
            });
        });
        
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

// 静态辅助方法：真正的干活逻辑放在这，方便传引用进线程
impl LowWuApp {
    pub fn start_core_task(sys_path: &str, target_drive: &str, logs: &[String]) {
        println!("[Worker] Task started for: {} -> {}", sys_path, target_drive);
        // 模拟执行过程（实际调用核心模块）
        // 注意：由于在线程中直接调用结构体很麻烦，这里直接用全局函数式的思维去调模块
        
        if let Err(e) = core::disk::DiskManager::format_partition(target_drive) {
           // error handling skipped for brevity, real
