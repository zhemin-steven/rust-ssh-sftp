use eframe::egui;
use std::sync::{Arc, Mutex};
use crate::config::{AppConfig, SavedConnection};
use crate::crypto::CryptoManager;

pub fn run_gui() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Rust SSH/SFTP Client"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Rust SSH/SFTP Client",
        options,
        Box::new(|cc| {
            // 设置中文字体
            setup_custom_fonts(&cc.egui_ctx);
            Box::new(SshGuiApp::new())
        }),
    )
}

/// 设置自定义字体以支持中文
fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    
    // 尝试加载系统中文字体
    // Windows 系统字体路径
    let font_paths = vec![
        r"C:\Windows\Fonts\msyh.ttc",      // 微软雅黑
        r"C:\Windows\Fonts\simsun.ttc",    // 宋体
        r"C:\Windows\Fonts\simhei.ttf",    // 黑体
    ];
    
    let mut font_loaded = false;
    for font_path in font_paths {
        if let Ok(font_data) = std::fs::read(font_path) {
            fonts.font_data.insert(
                "chinese_font".to_owned(),
                egui::FontData::from_owned(font_data),
            );
            
            // 将中文字体添加到所有字体族中，并设置为最高优先级
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese_font".to_owned());
            
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "chinese_font".to_owned());
            
            font_loaded = true;
            break;
        }
    }
    
    if !font_loaded {
        eprintln!("警告: 无法加载中文字体，中文可能无法正确显示");
    }
    
    ctx.set_fonts(fonts);
}

struct SshGuiApp {
    config: Arc<Mutex<AppConfig>>,
    selected_connection: Option<String>,
    
    // New connection form
    show_new_connection: bool,
    new_conn_name: String,
    new_conn_host: String,
    new_conn_port: String,
    new_conn_username: String,
    new_conn_password: String,
    new_conn_save_password: bool,
    
    // Master password
    master_password: String,
    show_master_password_dialog: bool,
    
    // Status messages
    status_message: String,
    error_message: String,
    
    // Connection state
    connecting: bool,
}

impl SshGuiApp {
    /// 创建新的 GUI 应用实例，自动加载配置
    fn new() -> Self {
        let config = AppConfig::load().unwrap_or_default();
        let status_message = if config.list_connections().is_empty() {
            String::new()
        } else {
            format!("已加载 {} 个连接", config.list_connections().len())
        };
        
        Self {
            config: Arc::new(Mutex::new(config)),
            selected_connection: None,
            show_new_connection: false,
            new_conn_name: String::new(),
            new_conn_host: String::new(),
            new_conn_port: "22".to_string(),
            new_conn_username: String::new(),
            new_conn_password: String::new(),
            new_conn_save_password: false,
            master_password: String::new(),
            show_master_password_dialog: false,
            status_message,
            error_message: String::new(),
            connecting: false,
        }
    }
    
    fn load_config(&mut self) {
        match AppConfig::load() {
            Ok(config) => {
                *self.config.lock().unwrap() = config;
                self.status_message = "配置加载成功".to_string();
            }
            Err(e) => {
                self.error_message = format!("加载配置失败: {}", e);
            }
        }
    }
    
    fn save_config(&mut self) {
        let config = self.config.lock().unwrap();
        if let Err(e) = config.save() {
            self.error_message = format!("保存配置失败: {}", e);
        } else {
            self.status_message = "配置保存成功".to_string();
        }
    }
    
    fn add_new_connection(&mut self) {
        // Validate inputs
        if self.new_conn_name.is_empty() || self.new_conn_host.is_empty() 
            || self.new_conn_username.is_empty() {
            self.error_message = "请填写所有必填字段".to_string();
            return;
        }
        
        let port: u16 = self.new_conn_port.parse().unwrap_or(22);
        
        let saved_conn = if self.new_conn_save_password && !self.new_conn_password.is_empty() {
            // Need master password
            if self.master_password.is_empty() {
                self.show_master_password_dialog = true;
                return;
            }
            
            // Create crypto manager
            match CryptoManager::new(&self.master_password) {
                Ok(crypto) => {
                    match crypto.encrypt(&self.new_conn_password) {
                        Ok(encrypted) => {
                            SavedConnection::new_password_with_encrypted(
                                self.new_conn_name.clone(),
                                self.new_conn_host.clone(),
                                port,
                                self.new_conn_username.clone(),
                                encrypted,
                            )
                        }
                        Err(e) => {
                            self.error_message = format!("加密密码失败: {}", e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    self.error_message = format!("创建加密管理器失败: {}", e);
                    return;
                }
            }
        } else {
            SavedConnection::new_password(
                self.new_conn_name.clone(),
                self.new_conn_host.clone(),
                port,
                self.new_conn_username.clone(),
            )
        };
        
        let mut config = self.config.lock().unwrap();
        config.add_connection(saved_conn);
        drop(config);
        
        self.save_config();
        self.show_new_connection = false;
        
        // Clear form
        self.new_conn_name.clear();
        self.new_conn_host.clear();
        self.new_conn_port = "22".to_string();
        self.new_conn_username.clear();
        self.new_conn_password.clear();
        self.new_conn_save_password = false;
        
        self.status_message = "连接添加成功".to_string();
    }
    
    fn delete_connection(&mut self, name: &str) {
        let mut config = self.config.lock().unwrap();
        if let Err(e) = config.remove_connection(name) {
            self.error_message = format!("删除连接失败: {}", e);
        } else {
            drop(config);
            self.save_config();
            self.status_message = format!("连接 '{}' 已删除", name);
            if self.selected_connection.as_deref() == Some(name) {
                self.selected_connection = None;
            }
        }
    }
    
    fn connect_to_selected(&mut self) {
        if let Some(conn_name) = &self.selected_connection {
            self.status_message = format!("正在打开终端连接到 '{}'...", conn_name);
            self.connecting = true;
            
            // 启动新的终端窗口进行连接
            match self.launch_terminal_connection(conn_name) {
                Ok(_) => {
                    self.status_message = format!("已启动终端连接到 '{}'", conn_name);
                }
                Err(e) => {
                    self.error_message = format!("启动终端失败: {}", e);
                }
            }
            
            self.connecting = false;
        }
    }
    
    /// 启动新的终端窗口并执行SSH连接
    fn launch_terminal_connection(&self, conn_name: &str) -> Result<(), String> {
        use std::process::Command;
        
        // 获取当前可执行文件的路径
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("无法获取可执行文件路径: {}", e))?;
        
        // 构建连接命令
        let connect_cmd = format!("{} connect {} -I", 
            exe_path.display(), 
            conn_name);
        
        // 在Windows上启动新的终端窗口
        #[cfg(target_os = "windows")]
        {
            // 使用 cmd.exe start 命令来打开新的 PowerShell 窗口
            // 需要对命令进行转义处理
            let escaped_cmd = connect_cmd.replace("\"", "\\\"");
            Command::new("cmd.exe")
                .args(&[
                    "/c",
                    "start",
                    "powershell.exe",
                    "-NoExit",
                    "-Command",
                    &escaped_cmd
                ])
                .spawn()
                .map_err(|e| format!("启动新终端失败: {}", e))?;
        }
        
        // 在Linux/Unix上启动新的终端窗口
        #[cfg(target_os = "linux")]
        {
            // 尝试使用常见的终端模拟器
            let terminals = vec![
                ("gnome-terminal", vec!["--", "bash", "-c", &format!("{}; exec bash", connect_cmd)]),
                ("konsole", vec!["-e", "bash", "-c", &format!("{}; exec bash", connect_cmd)]),
                ("xterm", vec!["-e", "bash", "-c", &format!("{}; exec bash", connect_cmd)]),
            ];
            
            let mut launched = false;
            for (terminal, args) in terminals {
                if let Ok(_) = Command::new(terminal)
                    .args(&args)
                    .spawn() {
                    launched = true;
                    break;
                }
            }
            
            if !launched {
                return Err("未找到可用的终端模拟器".to_string());
            }
        }
        
        // 在macOS上启动新的终端窗口
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .args(&[
                    "-a", "Terminal",
                    &exe_path.to_string_lossy().to_string(),
                    "connect",
                    conn_name,
                    "-I"
                ])
                .spawn()
                .map_err(|e| format!("启动终端失败: {}", e))?;
        }
        
        Ok(())
    }
}

impl eframe::App for SshGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("新建连接").clicked() {
                        self.show_new_connection = true;
                        ui.close_menu();
                    }
                    if ui.button("刷新").clicked() {
                        self.load_config();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("退出").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                ui.menu_button("帮助", |ui| {
                    if ui.button("关于").clicked() {
                        self.status_message = "Rust SSH/SFTP Client v0.1.0\n类似 FinalShell 的跨平台终端工具".to_string();
                        ui.close_menu();
                    }
                });
            });
        });
        
        // Bottom panel for status
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if !self.status_message.is_empty() {
                    ui.label(egui::RichText::new(&self.status_message).color(egui::Color32::GREEN));
                }
                if !self.error_message.is_empty() {
                    ui.label(egui::RichText::new(&self.error_message).color(egui::Color32::RED));
                }
            });
        });
        
        // Main panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("SSH 连接管理");
            ui.separator();
            
            // Connection list
            ui.horizontal(|ui| {
                ui.label("已保存的连接:");
                if ui.button("➕ 新建").clicked() {
                    self.show_new_connection = true;
                }
            });
            
            ui.separator();

            // Collect connection data first to avoid borrow issues
            let connections_data: Vec<(String, String, String, u16, bool)> = {
                let config = self.config.lock().unwrap();
                config.list_connections()
                    .iter()
                    .map(|conn| (
                        conn.name.clone(),
                        conn.username.clone(),
                        conn.host.clone(),
                        conn.port,
                        conn.has_saved_password(),
                    ))
                    .collect()
            };

            let mut connection_to_delete: Option<String> = None;

            if connections_data.is_empty() {
                ui.label("没有保存的连接");
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (name, username, host, port, has_password) in &connections_data {
                        ui.horizontal(|ui| {
                            let is_selected = self.selected_connection.as_deref() == Some(name.as_str());

                            if ui.selectable_label(is_selected, name).clicked() {
                                self.selected_connection = Some(name.clone());
                                self.error_message.clear();
                            }

                            ui.label(format!("{}@{}:{}", username, host, port));

                            if *has_password {
                                ui.label("🔒");
                            }

                            if ui.button("🗑").clicked() {
                                connection_to_delete = Some(name.clone());
                            }
                        });
                    }
                });
            }

            // Delete connection if requested
            if let Some(name) = connection_to_delete {
                self.delete_connection(&name);
            }
            
            ui.separator();
            
            // Connection buttons
            ui.horizontal(|ui| {
                if ui.button("连接").clicked() {
                    self.connect_to_selected();
                }
                
                ui.label("💡 提示: 点击连接按钮将自动打开新终端窗口");
            });
        });
        
        // New connection dialog
        if self.show_new_connection {
            egui::Window::new("新建连接")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("连接名称:");
                    ui.text_edit_singleline(&mut self.new_conn_name);
                    
                    ui.label("主机地址:");
                    ui.text_edit_singleline(&mut self.new_conn_host);
                    
                    ui.label("端口:");
                    ui.text_edit_singleline(&mut self.new_conn_port);
                    
                    ui.label("用户名:");
                    ui.text_edit_singleline(&mut self.new_conn_username);
                    
                    ui.checkbox(&mut self.new_conn_save_password, "保存密码");
                    
                    if self.new_conn_save_password {
                        ui.label("密码:");
                        ui.add(egui::TextEdit::singleline(&mut self.new_conn_password).password(true));
                        
                        ui.label("主密码:");
                        ui.add(egui::TextEdit::singleline(&mut self.master_password).password(true));
                    }
                    
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("添加").clicked() {
                            self.add_new_connection();
                        }
                        if ui.button("取消").clicked() {
                            self.show_new_connection = false;
                        }
                    });
                });
        }
    }
}

