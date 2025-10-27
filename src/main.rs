mod cli;
mod config;
mod crypto;
mod gui;
mod interactive_menu;
mod sftp;
mod ssh;
mod ssh_russh;
mod terminal;
mod terminal_russh;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands, ConfigCommands, SftpCommands};
use colored::Colorize;
use config::{AppConfig, SavedConnection};
use crypto::CryptoManager;
use sftp::SftpClient;
use ssh::{AuthMethod, SshClient, SshConfig};
use terminal::{InteractiveTerminal, SimpleShell};

#[tokio::main]
async fn main() {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("{} {}", "错误:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Connect {
            target,
            port,
            interactive,
            identity_file,
            save_password,
            save_as,
        } => {
            // 如果没有提供 target，显示交互式菜单
            let actual_target = if let Some(t) = target {
                t
            } else {
                match interactive_menu::show_connection_menu()? {
                    Some(t) => t,
                    None => {
                        println!("{}", "已取消连接".yellow());
                        return Ok(());
                    }
                }
            };

            // 检查是否是手动输入的连接（通过环境变量）
            let actual_port = if let Ok(manual_port) = std::env::var("MANUAL_CONNECTION_PORT") {
                manual_port.parse().unwrap_or(port)
            } else {
                port
            };

            let actual_save_password = if let Ok(manual_save) = std::env::var("MANUAL_CONNECTION_SAVE") {
                manual_save == "1" || save_password
            } else {
                save_password
            };

            let actual_save_as = if let Ok(manual_name) = std::env::var("MANUAL_CONNECTION_NAME") {
                Some(manual_name)
            } else {
                save_as
            };

            // 清理环境变量
            std::env::remove_var("MANUAL_CONNECTION_PORT");
            std::env::remove_var("MANUAL_CONNECTION_SAVE");
            std::env::remove_var("MANUAL_CONNECTION_NAME");

            handle_connect_command(
                &actual_target,
                actual_port,
                interactive,
                identity_file,
                actual_save_password,
                actual_save_as,
            ).await?;
        }
        
        Commands::Exec {
            target,
            command,
            port,
            identity_file,
        } => {
            let ssh_config = parse_target(&target, port, identity_file)?;
            let client = SshClient::connect(ssh_config)?;
            let terminal = InteractiveTerminal::new(&client);
            terminal.exec_command(&command)?;
        }
        
        Commands::Sftp { action } => {
            handle_sftp_command(action)?;
        }

        Commands::Config { action } => {
            handle_config_command(action)?;
        }

        Commands::Gui => {
            // GUI mode - run in blocking mode
            return gui::run_gui().map_err(|e| anyhow::anyhow!("GUI 错误: {}", e));
        }
    }

    Ok(())
}

fn handle_sftp_command(action: SftpCommands) -> Result<()> {
    match action {
        SftpCommands::Upload {
            target,
            local_path,
            remote_path,
            port,
            identity_file,
            no_progress,
        } => {
            let ssh_config = parse_target(&target, port, identity_file)?;
            let client = SshClient::connect(ssh_config)?;
            let sftp = SftpClient::new(&client)?;
            sftp.upload_file(&local_path, &remote_path, !no_progress)?;
            println!("{}", "上传成功!".green().bold());
        }
        
        SftpCommands::Download {
            target,
            remote_path,
            local_path,
            port,
            identity_file,
            no_progress,
        } => {
            let ssh_config = parse_target(&target, port, identity_file)?;
            let client = SshClient::connect(ssh_config)?;
            let sftp = SftpClient::new(&client)?;
            sftp.download_file(&remote_path, &local_path, !no_progress)?;
            println!("{}", "下载成功!".green().bold());
        }
        
        SftpCommands::List {
            target,
            remote_path,
            port,
            identity_file,
        } => {
            let ssh_config = parse_target(&target, port, identity_file)?;
            let client = SshClient::connect(ssh_config)?;
            let sftp = SftpClient::new(&client)?;
            let files = sftp.list_dir(&remote_path)?;
            
            println!("\n{} {}\n", "目录:".cyan().bold(), remote_path);
            println!("{:<40} {:>12} {}", "名称", "大小", "类型");
            println!("{}", "-".repeat(60));
            
            for file in files {
                let file_type = if file.is_dir { "目录".blue() } else { "文件".normal() };
                let size = if file.is_dir {
                    "-".to_string()
                } else {
                    format_size(file.size)
                };
                println!("{:<40} {:>12} {}", file.name, size, file_type);
            }
        }
        
        SftpCommands::Mkdir {
            target,
            remote_path,
            port,
            identity_file,
        } => {
            let ssh_config = parse_target(&target, port, identity_file)?;
            let client = SshClient::connect(ssh_config)?;
            let sftp = SftpClient::new(&client)?;
            sftp.mkdir(&remote_path)?;
            println!("{} 目录创建成功: {}", "✓".green().bold(), remote_path);
        }
        
        SftpCommands::Remove {
            target,
            remote_path,
            port,
            identity_file,
        } => {
            let ssh_config = parse_target(&target, port, identity_file)?;
            let client = SshClient::connect(ssh_config)?;
            let sftp = SftpClient::new(&client)?;
            sftp.remove_file(&remote_path)?;
            println!("{} 文件删除成功: {}", "✓".green().bold(), remote_path);
        }
    }
    
    Ok(())
}

fn handle_config_command(action: ConfigCommands) -> Result<()> {
    let mut config = AppConfig::load()?;
    
    match action {
        ConfigCommands::Add {
            name,
            host,
            username,
            port,
            use_key,
            identity_file,
            public_key,
        } => {
            let connection = if use_key {
                let private_key = identity_file
                    .context("使用公钥认证时必须提供 --identity-file")?;
                SavedConnection::new_publickey(name.clone(), host, port, username, private_key, public_key)
            } else {
                SavedConnection::new_password(name.clone(), host, port, username)
            };
            
            config.add_connection(connection);
            config.save()?;
            println!("{} 连接 '{}' 已添加", "✓".green().bold(), name);
        }
        
        ConfigCommands::List => {
            let connections = config.list_connections();

            if connections.is_empty() {
                println!("没有保存的连接");
                return Ok(());
            }

            println!("\n{}\n", "保存的连接:".cyan().bold());

            let first_name = connections.first().map(|c| c.name.clone());

            for conn in connections {
                let is_default = config.default_connection.as_deref() == Some(&conn.name);
                let marker = if is_default { "*" } else { " " };
                let password_marker = if conn.has_saved_password() { "🔑" } else { "" };

                println!("{} [{}] {}@{}:{} ({}) {}",
                    marker.green().bold(),
                    conn.name.yellow().bold(),
                    conn.username.cyan(),
                    conn.host,
                    conn.port,
                    conn.auth_type,
                    password_marker);
            }

            println!("\n{}", "提示:".yellow().bold());
            println!("  {} 表示默认连接", "*".green().bold());
            println!("  {} 表示已保存密码", "🔑");
            println!("  使用 {} 连接，例如: connect {}",
                "[连接名称]".yellow(),
                first_name.as_deref().unwrap_or("myserver"));
        }
        
        ConfigCommands::Remove { name } => {
            config.remove_connection(&name)?;
            config.save()?;
            println!("{} 连接 '{}' 已删除", "✓".green().bold(), name);
        }
        
        ConfigCommands::SetDefault { name } => {
            config.set_default(&name)?;
            config.save()?;
            println!("{} '{}' 已设为默认连接", "✓".green().bold(), name);
        }
        
        ConfigCommands::Show { name } => {
            let conn = config.get_connection(&name)
                .context(format!("连接 '{}' 不存在", name))?;
            
            println!("\n{} {}\n", "连接详情:".cyan().bold(), name);
            println!("  主机:     {}", conn.host);
            println!("  端口:     {}", conn.port);
            println!("  用户名:   {}", conn.username);
            println!("  认证方式: {}", conn.auth_type);
            
            if let Some(ref key) = conn.private_key_path {
                println!("  私钥:     {}", key);
            }
            if let Some(ref key) = conn.public_key_path {
                println!("  公钥:     {}", key);
            }
        }
        
        ConfigCommands::ShowPassword { name } => {
            // 检查是否有保存的密码
            let connections_with_password: Vec<_> = if let Some(ref name) = name {
                // 显示指定连接的密码
                let conn = config.get_connection(name)
                    .context(format!("连接 '{}' 不存在", name))?;
                if !conn.has_saved_password() {
                    anyhow::bail!("连接 '{}' 没有保存密码", name);
                }
                vec![conn.clone()]
            } else {
                // 显示所有有保存密码的连接
                config.list_connections()
                    .into_iter()
                    .filter(|c| c.has_saved_password())
                    .cloned()
                    .collect()
            };

            if connections_with_password.is_empty() {
                println!("{}", "没有保存密码的连接".yellow());
                return Ok(());
            }

            // 检查主密码是否存在
            if !CryptoManager::has_master_password() {
                anyhow::bail!("未设置主密码，无法解密");
            }

            // 获取主密码
            println!("{}", "需要主密码来解密保存的密码".yellow().bold());
            let master_password = rpassword::prompt_password("请输入主密码: ")
                .context("无法读取主密码")?;

            if master_password.is_empty() {
                anyhow::bail!("主密码不能为空");
            }

            // 创建加密管理器
            let crypto_manager = CryptoManager::new(&master_password)
                .context("创建加密管理器失败")?;

            // 解密并显示密码
            println!("\n{}\n", "已保存的密码:".cyan().bold());

            for conn in connections_with_password {
                let password_info = if conn.auth_type == "password" {
                    // 密码认证
                    if let Some(ref encrypted) = conn.encrypted_password {
                        match crypto_manager.decrypt(encrypted) {
                            Ok(password) => format!("{}", password.green()),
                            Err(e) => format!("{}", format!("解密失败: {}", e).red()),
                        }
                    } else {
                        "无密码".red().to_string()
                    }
                } else if conn.auth_type == "publickey" {
                    // 公钥认证 - 显示私钥密码
                    if let Some(ref encrypted) = conn.encrypted_passphrase {
                        match crypto_manager.decrypt(encrypted) {
                            Ok(passphrase) => format!("{}", passphrase.green()),
                            Err(e) => format!("{}", format!("解密失败: {}", e).red()),
                        }
                    } else {
                        "无私钥密码".red().to_string()
                    }
                } else {
                    "未知认证类型".red().to_string()
                };

                println!("  [{}]", conn.name.yellow().bold());
                println!("    主机:     {}@{}:{}", conn.username, conn.host, conn.port);
                println!("    认证方式: {}", conn.auth_type);
                
                if conn.auth_type == "password" {
                    println!("    密码:     {}", password_info);
                } else if conn.auth_type == "publickey" {
                    if let Some(ref key) = conn.private_key_path {
                        println!("    私钥:     {}", key);
                    }
                    println!("    私钥密码: {}", password_info);
                }
                println!();
            }

            println!("{}", "⚠️  请注意保护好这些密码信息！".yellow().bold());
        }
    }
    
    Ok(())
}

/// 处理连接命令
async fn handle_connect_command(
    target: &str,
    port: u16,
    interactive: bool,
    identity_file: Option<String>,
    save_password: bool,
    save_as: Option<String>,
) -> Result<()> {
    // 使用 russh 进行交互式连接
    if interactive {
        return handle_interactive_connect_russh(target, port, identity_file, save_password, save_as).await;
    }

    // 非交互式模式继续使用旧代码
    handle_connect_command_legacy(target, port, interactive, identity_file, save_password, save_as)
}

/// 使用 russh 处理交互式连接
async fn handle_interactive_connect_russh(
    target: &str,
    port: u16,
    identity_file: Option<String>,
    save_password: bool,
    save_as: Option<String>,
) -> Result<()> {
    use ssh_russh::{AuthMethod as RusshAuthMethod, RusshClient, SshConfig as RusshSshConfig};
    use terminal_russh::InteractiveTerminal as RusshInteractiveTerminal;

    // 加载配置以检查是否有保存的连接
    let mut config = AppConfig::load()?;
    let mut actual_port = port;
    let mut password_to_save: Option<String> = None;
    let mut connection_info: Option<(String, String, u16, String)> = None; // (name, host, port, username)

    // 检查是否从保存的连接加载
    let saved_conn = config.get_connection(target);

    // 获取认证信息
    let (actual_host, actual_username, auth) = if let Some(saved_conn) = saved_conn {
        println!("{} 使用保存的连接: {}", "→".cyan(), saved_conn.name.bold());
        let host = saved_conn.host.clone();
        actual_port = saved_conn.port;
        let username = saved_conn.username.clone();

        // 尝试使用已保存的密码
        let auth = if saved_conn.has_saved_password() && identity_file.is_none() {
            println!("{} 检测到已保存的密码", "✓".green());

            // 获取主密码
            let is_first_time = !CryptoManager::has_master_password();
            let master_password = CryptoManager::get_master_password(is_first_time)?;
            let crypto_manager = CryptoManager::new(&master_password)?;

            // 尝试解密密码
            match saved_conn.to_ssh_config_with_decryption(&crypto_manager, None, None) {
                Ok(ssh_config) => {
                    println!("{} 使用已保存的密码", "✓".green());
                    // 从 ssh_config 提取密码
                    if let AuthMethod::Password(pwd) = ssh_config.auth {
                        RusshAuthMethod::Password(pwd)
                    } else {
                        // 不应该发生，但以防万一
                        let password = rpassword::prompt_password(format!("{}@{} 的密码: ", username, host))?;
                        RusshAuthMethod::Password(password)
                    }
                }
                Err(e) => {
                    println!("{} 解密失败: {}", "✗".red(), e);
                    println!("{} 请手动输入密码", "→".yellow());
                    let password = rpassword::prompt_password(format!("{}@{} 的密码: ", username, host))?;

                    if save_password {
                        password_to_save = Some(password.clone());
                        connection_info = Some((
                            saved_conn.name.clone(),
                            saved_conn.host.clone(),
                            saved_conn.port,
                            saved_conn.username.clone(),
                        ));
                    }

                    RusshAuthMethod::Password(password)
                }
            }
        } else if let Some(key_path) = identity_file {
            RusshAuthMethod::PublicKey(key_path)
        } else {
            // 没有保存的密码，手动输入
            let password = rpassword::prompt_password(format!("{}@{} 的密码: ", username, host))?;

            if save_password {
                password_to_save = Some(password.clone());
                connection_info = Some((
                    saved_conn.name.clone(),
                    saved_conn.host.clone(),
                    saved_conn.port,
                    saved_conn.username.clone(),
                ));
            }

            RusshAuthMethod::Password(password)
        };

        (host, username, auth)
    } else {
        // 没有保存的连接，解析目标
        let (username, host) = if target.contains('@') {
            let parts: Vec<&str> = target.split('@').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("无效的目标格式，应为 user@host"));
            }
            (parts[0].to_string(), parts[1].to_string())
        } else {
            return Err(anyhow::anyhow!("目标必须包含用户名，格式: user@host"));
        };

        let auth = if let Some(key_path) = identity_file {
            RusshAuthMethod::PublicKey(key_path)
        } else {
            let password = rpassword::prompt_password(format!("{}@{} 的密码: ", username, host))?;

            if save_password {
                let conn_name = save_as.unwrap_or_else(|| format!("{}@{}", username, host));
                password_to_save = Some(password.clone());
                connection_info = Some((
                    conn_name,
                    host.clone(),
                    actual_port,
                    username.clone(),
                ));
            }

            RusshAuthMethod::Password(password)
        };

        (host, username, auth)
    };

    // 创建配置
    let ssh_config = RusshSshConfig::new(actual_host.clone(), actual_port, actual_username.clone(), auth);

    // 连接
    println!("{} 正在连接到 {}@{}:{}...", "→".cyan(), actual_username, actual_host, actual_port);
    let mut client = RusshClient::new(ssh_config);
    client.connect().await?;
    println!("{} 连接成功!", "✓".green());

    // 如果需要保存密码，在连接成功后保存
    if let (Some(password), Some((name, host, port, username))) = (password_to_save, connection_info) {
        println!("{} 正在保存密码...", "→".cyan());

        let is_first_time = !CryptoManager::has_master_password();
        let master_password = CryptoManager::get_master_password(is_first_time)?;
        let crypto_manager = CryptoManager::new(&master_password)?;

        // 加密密码
        let encrypted_password = crypto_manager.encrypt(&password)?;

        // 创建保存的连接
        let saved_conn = SavedConnection::new_password_with_encrypted(
            name.clone(),
            host,
            port,
            username,
            encrypted_password,
        );

        config.add_connection(saved_conn);
        config.save()?;

        println!("{} 密码已保存到连接: {}", "✓".green(), name.bold());
    }

    // 启动交互式终端
    let mut terminal = RusshInteractiveTerminal::new(&mut client);
    terminal.start_shell().await?;

    // 断开连接
    client.disconnect().await?;

    Ok(())
}

/// 旧的连接处理函数（保留用于非交互式模式）
fn handle_connect_command_legacy(
    target: &str,
    port: u16,
    interactive: bool,
    identity_file: Option<String>,
    save_password: bool,
    save_as: Option<String>,
) -> Result<()> {
    let mut config = AppConfig::load()?;
    let crypto: Option<CryptoManager> = None;
    let mut password_to_save: Option<String> = None;
    let mut connection_info: Option<(String, String, u16, String)> = None; // (name, host, port, username)

    // 检查是否从保存的连接加载
    let ssh_config = if let Some(saved_conn) = config.get_connection(target) {
        // 从保存的连接加载
        println!("{} 使用保存的连接: {}", "→".cyan(), saved_conn.name.bold());

        let ssh_config = if saved_conn.has_saved_password() {
            // 有保存的密码，尝试自动填充
            println!("{} 检测到已保存的密码", "✓".green());

            // 获取主密码
            let is_first_time = !CryptoManager::has_master_password();
            let master_password = CryptoManager::get_master_password(is_first_time)?;
            let crypto_manager = CryptoManager::new(&master_password)?;

            // 尝试解密并连接
            match saved_conn.to_ssh_config_with_decryption(&crypto_manager, None, None) {
                Ok(config) => {
                    println!("{} 使用已保存的密码", "✓".green());
                    config
                }
                Err(e) => {
                    println!("{} 解密失败: {}", "✗".red(), e);
                    println!("{} 请手动输入密码", "→".yellow());

                    // 手动输入密码
                    let password = if saved_conn.auth_type == "password" {
                        Some(rpassword::prompt_password(format!("{}@{} 的密码: ", saved_conn.username, saved_conn.host))?)
                    } else {
                        None
                    };

                    let passphrase = if saved_conn.auth_type == "publickey" {
                        let pp = rpassword::prompt_password("私钥密码（如果没有请直接回车）: ")?;
                        if pp.is_empty() { None } else { Some(pp) }
                    } else {
                        None
                    };

                    saved_conn.to_ssh_config(password, passphrase)?
                }
            }
        } else {
            // 没有保存的密码，手动输入
            let password = if saved_conn.auth_type == "password" {
                let pwd = rpassword::prompt_password(format!("{}@{} 的密码: ", saved_conn.username, saved_conn.host))?;
                if save_password {
                    password_to_save = Some(pwd.clone());
                    connection_info = Some((
                        saved_conn.name.clone(),
                        saved_conn.host.clone(),
                        saved_conn.port,
                        saved_conn.username.clone(),
                    ));
                }
                Some(pwd)
            } else {
                None
            };

            let passphrase = if saved_conn.auth_type == "publickey" {
                let pp = rpassword::prompt_password("私钥密码（如果没有请直接回车）: ")?;
                if pp.is_empty() { None } else { Some(pp) }
            } else {
                None
            };

            saved_conn.to_ssh_config(password, passphrase)?
        };

        ssh_config
    } else {
        // 解析 user@host 格式
        if let Some((username, host)) = target.split_once('@') {
            let auth = if let Some(key_path) = identity_file {
                let passphrase = rpassword::prompt_password("私钥密码（如果没有请直接回车）: ")?;
                let passphrase = if passphrase.is_empty() { None } else { Some(passphrase) };

                AuthMethod::PublicKey {
                    public_key: None,
                    private_key: key_path,
                    passphrase,
                }
            } else {
                let password = rpassword::prompt_password(format!("{}@{} 的密码: ", username, host))?;
                if save_password || save_as.is_some() {
                    password_to_save = Some(password.clone());
                    let conn_name = save_as.clone().unwrap_or_else(|| format!("{}@{}", username, host));
                    connection_info = Some((conn_name, host.to_string(), port, username.to_string()));
                }
                AuthMethod::Password(password)
            };

            SshConfig {
                host: host.to_string(),
                port,
                username: username.to_string(),
                auth,
            }
        } else {
            anyhow::bail!("无效的目标格式。请使用 'user@host' 或保存的连接名称");
        }
    };

    // 连接到服务器
    println!("{} 正在连接到 {}@{}:{}...", "→".cyan(), ssh_config.username, ssh_config.host, ssh_config.port);
    let client = SshClient::connect(ssh_config)?;
    println!("{} 连接成功!", "✓".green().bold());

    // 如果需要保存密码
    if let (Some(password), Some((name, host, port, username))) = (password_to_save, connection_info) {
        println!("\n{} 正在保存密码...", "→".cyan());

        // 获取或创建加密管理器
        let crypto_manager = if let Some(c) = crypto {
            c
        } else {
            let is_first_time = !CryptoManager::has_master_password();
            let master_password = CryptoManager::get_master_password(is_first_time)?;
            CryptoManager::new(&master_password)?
        };

        // 加密密码
        let encrypted_password = crypto_manager.encrypt(&password)?;

        // 创建或更新连接配置
        let saved_conn = SavedConnection::new_password_with_encrypted(
            name.clone(),
            host,
            port,
            username,
            encrypted_password,
        );

        config.add_connection(saved_conn);
        config.save()?;

        println!("{} 密码已加密保存到连接 '{}'", "✓".green().bold(), name);
    }

    // 启动 shell
    if interactive {
        let terminal = InteractiveTerminal::new(&client);
        terminal.start_shell()?;
    } else {
        let shell = SimpleShell::new(&client);
        shell.start()?;
    }

    Ok(())
}

/// 解析目标字符串（连接名称或 user@host 格式）
fn parse_target(target: &str, port: u16, identity_file: Option<String>) -> Result<SshConfig> {
    // 首先尝试从配置中加载
    let config = AppConfig::load()?;
    
    if let Some(saved_conn) = config.get_connection(target) {
        // 从保存的连接加载
        let password = if saved_conn.auth_type == "password" {
            Some(rpassword::prompt_password(format!("{}@{} 的密码: ", saved_conn.username, saved_conn.host))?)
        } else {
            None
        };
        
        let passphrase = if saved_conn.auth_type == "publickey" {
            let pp = rpassword::prompt_password("私钥密码（如果没有请直接回车）: ")?;
            if pp.is_empty() { None } else { Some(pp) }
        } else {
            None
        };
        
        return saved_conn.to_ssh_config(password, passphrase);
    }
    
    // 解析 user@host 格式
    if let Some((username, host)) = target.split_once('@') {
        let auth = if let Some(key_path) = identity_file {
            let passphrase = rpassword::prompt_password("私钥密码（如果没有请直接回车）: ")?;
            let passphrase = if passphrase.is_empty() { None } else { Some(passphrase) };
            
            AuthMethod::PublicKey {
                public_key: None,
                private_key: key_path,
                passphrase,
            }
        } else {
            let password = rpassword::prompt_password(format!("{}@{} 的密码: ", username, host))?;
            AuthMethod::Password(password)
        };
        
        return Ok(SshConfig {
            host: host.to_string(),
            port,
            username: username.to_string(),
            auth,
        });
    }
    
    anyhow::bail!("无效的目标格式。请使用 'user@host' 或保存的连接名称")
}

/// 格式化文件大小
fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_idx = 0;
    
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    
    format!("{:.2} {}", size, UNITS[unit_idx])
}

