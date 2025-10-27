use anyhow::{Context, Result};
use colored::Colorize;
use std::io::{self, Write};
use crate::config::{AppConfig, SavedConnection};

/// 显示交互式连接选择菜单
pub fn show_connection_menu() -> Result<Option<String>> {
    let config = AppConfig::load()?;
    let connections = config.list_connections();
    
    if connections.is_empty() {
        println!("{}", "没有保存的连接。".yellow());
        println!("\n{}", "提示：".cyan().bold());
        println!("  1. 使用 {} 添加新连接", "config add".green());
        println!("  2. 或直接使用 {} 连接", "connect user@host -I --save-password --save-as \"name\"".green());
        return Ok(None);
    }
    
    // 显示连接列表
    println!("\n{}", "=== 已保存的连接 ===".cyan().bold());
    println!();
    
    for (idx, conn) in connections.iter().enumerate() {
        let num = format!("[{}]", idx + 1).cyan().bold();
        let name = conn.name.bold();
        let info = format!("{}@{}:{}", conn.username, conn.host, conn.port).dimmed();
        let pwd_indicator = if conn.has_saved_password() {
            "🔒".green()
        } else {
            "🔓".yellow()
        };
        
        println!("  {} {} {} {}", num, name, info, pwd_indicator);
    }
    
    println!();
    println!("  {} 手动输入连接信息", "[0]".cyan().bold());
    println!("  {} 退出", "[q]".cyan().bold());
    println!();
    
    // 获取用户选择
    loop {
        print!("{} ", format!("请选择连接 [1-{}, 0=手动, q=退出]:", connections.len()).green().bold());
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.eq_ignore_ascii_case("q") {
            return Ok(None);
        }
        
        if input == "0" {
            // 手动输入
            return get_manual_connection_info();
        }
        
        // 尝试解析为数字
        if let Ok(choice) = input.parse::<usize>() {
            if choice >= 1 && choice <= connections.len() {
                let selected = connections[choice - 1];
                println!("\n{} 已选择: {}", "✓".green(), selected.name.bold());
                return Ok(Some(selected.name.clone()));
            }
        }
        
        println!("{} 无效的选择，请重试", "✗".red());
    }
}

/// 手动输入连接信息
fn get_manual_connection_info() -> Result<Option<String>> {
    println!("\n{}", "=== 手动输入连接信息 ===".cyan().bold());
    
    // 获取主机
    print!("{} ", "主机地址:".green());
    io::stdout().flush()?;
    let mut host = String::new();
    io::stdin().read_line(&mut host)?;
    let host = host.trim();
    
    if host.is_empty() {
        return Ok(None);
    }
    
    // 获取用户名
    print!("{} ", "用户名:".green());
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim();
    
    if username.is_empty() {
        return Ok(None);
    }
    
    // 获取端口
    print!("{} [默认: 22]: ", "端口".green());
    io::stdout().flush()?;
    let mut port_str = String::new();
    io::stdin().read_line(&mut port_str)?;
    let port_str = port_str.trim();
    let port: u16 = if port_str.is_empty() {
        22
    } else {
        port_str.parse().context("无效的端口号")?
    };
    
    // 询问是否保存
    print!("{} [y/N]: ", "是否保存此连接?".green());
    io::stdout().flush()?;
    let mut save_choice = String::new();
    io::stdin().read_line(&mut save_choice)?;
    let should_save = save_choice.trim().eq_ignore_ascii_case("y");
    
    let connection_name = if should_save {
        print!("{} [默认: {}@{}]: ", "连接名称".green(), username, host);
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        let name = name.trim();
        
        if name.is_empty() {
            format!("{}@{}", username, host)
        } else {
            name.to_string()
        }
    } else {
        format!("{}@{}", username, host)
    };
    
    // 返回格式化的连接字符串，包含端口信息
    // 我们将通过环境变量或其他方式传递端口和保存选项
    std::env::set_var("MANUAL_CONNECTION_PORT", port.to_string());
    std::env::set_var("MANUAL_CONNECTION_SAVE", if should_save { "1" } else { "0" });
    std::env::set_var("MANUAL_CONNECTION_NAME", &connection_name);
    
    Ok(Some(format!("{}@{}", username, host)))
}

/// 显示连接详情
#[allow(dead_code)]
pub fn show_connection_details(conn: &SavedConnection) {
    println!("\n{}", "=== 连接详情 ===".cyan().bold());
    println!("  {}: {}", "名称".bold(), conn.name);
    println!("  {}: {}", "主机".bold(), conn.host);
    println!("  {}: {}", "端口".bold(), conn.port);
    println!("  {}: {}", "用户名".bold(), conn.username);
    println!("  {}: {}", "认证方式".bold(), conn.auth_type);
    
    if conn.has_saved_password() {
        println!("  {}: {}", "密码".bold(), "已保存（加密）".green());
    } else {
        println!("  {}: {}", "密码".bold(), "未保存".yellow());
    }
    
    if let Some(key_path) = &conn.private_key_path {
        println!("  {}: {}", "私钥".bold(), key_path);
    }
    
    println!();
}

