use anyhow::{Context, Result};
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use log::{debug, error, info};
use std::io::{self, Read, Write};
use std::thread;
use std::time::Duration;

use crate::ssh::SshClient;

/// 交互式 SSH 终端
pub struct InteractiveTerminal<'a> {
    ssh_client: &'a SshClient,
}

impl<'a> InteractiveTerminal<'a> {
    /// 创建交互式终端
    pub fn new(ssh_client: &'a SshClient) -> Self {
        Self { ssh_client }
    }
    
    /// 启动交互式 shell 会话
    pub fn start_shell(&self) -> Result<()> {
        info!("启动交互式 shell");

        // 创建 SSH 通道
        let mut channel = self.ssh_client.session().channel_session()
            .context("无法创建 SSH 通道")?;

        // 获取终端大小
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));

        // 请求 PTY，使用 xterm 而不是 xterm-256color
        // 这样可以减少一些不必要的控制序列
        channel.request_pty("xterm", None, Some((cols as u32, rows as u32, 0, 0)))
            .context("无法请求 PTY")?;

        // 设置环境变量
        let _ = channel.setenv("TERM", "xterm");

        // 启动 shell
        channel.shell()
            .context("无法启动 shell")?;

        println!("=== 交互式 SSH Shell ===");
        println!("连接到: {}@{}",
            self.ssh_client.config().username,
            self.ssh_client.config().host);
        println!("按 Ctrl+D 或输入 'exit' 退出");
        println!("========================\n");

        debug!("准备启用原始模式");
        // 启用原始模式
        enable_raw_mode().context("无法启用原始模式")?;
        debug!("原始模式已启用");

        debug!("准备进入 shell 循环");
        let result = self.run_shell_loop(&mut channel);
        debug!("shell 循环已退出");

        // 恢复终端
        disable_raw_mode().context("无法禁用原始模式")?;

        result
    }
    
    /// 运行 shell 循环
    fn run_shell_loop(&self, channel: &mut ssh2::Channel) -> Result<()> {
        debug!("进入 run_shell_loop");

        // 克隆通道用于读取线程
        debug!("准备克隆通道");
        let mut channel_clone = channel.stream(0);
        debug!("通道已克隆");

        // 启动读取线程（从 SSH 读取并输出到终端）
        debug!("准备启动读取线程");
        let read_handle = thread::spawn(move || {
            debug!("读取线程已启动");
            let mut buffer = [0u8; 8192];

            loop {
                match channel_clone.read(&mut buffer) {
                    Ok(0) => {
                        debug!("读取线程: 收到 EOF");
                        break;
                    }
                    Ok(n) => {
                        debug!("读取线程: 读取到 {} 字节", n);

                        // 过滤掉 CPR (Cursor Position Report) 等控制序列
                        let filtered = filter_control_sequences(&buffer[..n]);

                        // 输出到标准输出
                        if !filtered.is_empty() {
                            if let Err(e) = io::stdout().write_all(&filtered) {
                                error!("写入标准输出失败: {}", e);
                                break;
                            }
                            if let Err(e) = io::stdout().flush() {
                                error!("刷新标准输出失败: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("从 SSH 读取失败: {}", e);
                        break;
                    }
                }
            }
        });
        debug!("读取线程已启动完成");

        // 主循环（使用两个线程：一个读取 stdin，一个写入 SSH）
        debug!("准备进入主循环");

        use std::sync::mpsc;

        // 创建通道用于线程间通信
        let (tx, rx) = mpsc::channel::<u8>();

        // 启动 stdin 读取线程
        let _stdin_handle = thread::spawn(move || {
            use std::io::stdin;
            let mut stdin = stdin();
            let mut input_buffer = [0u8; 1];

            loop {
                match stdin.read(&mut input_buffer) {
                    Ok(1) => {
                        let byte = input_buffer[0];
                        debug!("stdin 线程: 读取到字节 {} (0x{:02x})", byte, byte);
                        if tx.send(byte).is_err() {
                            debug!("stdin 线程: 发送失败，退出");
                            break;
                        }
                    }
                    Ok(0) => {
                        debug!("stdin 线程: EOF");
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("stdin 线程: 读取失败: {}", e);
                        break;
                    }
                }
            }
        });

        // 主线程：接收字节并发送到 SSH
        let mut byte_count = 0;
        loop {
            // 使用超时接收，这样可以定期检查通道状态
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(byte) => {
                    byte_count += 1;
                    debug!("主循环: 收到字节 #{}: {} (0x{:02x})", byte_count, byte, byte);

                    // 检查 Ctrl+D (0x04) 或 Ctrl+C (0x03)
                    if byte == 0x04 || byte == 0x03 {
                        debug!("检测到 Ctrl+D/C，退出");
                        break;
                    }

                    // 过滤掉 CPR 序列的开始（ESC）
                    if byte == 0x1b {
                        debug!("主循环: 跳过 ESC 字节（可能是 CPR）");
                        continue;
                    }

                    // 发送字节到 SSH
                    debug!("主循环: 准备发送字节到 SSH");
                    match channel.write(&[byte]) {
                        Ok(n) => {
                            debug!("主循环: write 成功，写入了 {} 字节", n);
                        }
                        Err(e) => {
                            error!("主循环: write 失败: {}", e);
                            break;
                        }
                    }
                    debug!("主循环: 字节已发送，继续循环");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // 超时，继续循环
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    debug!("主循环: stdin 线程已断开");
                    break;
                }
            }

            // 检查通道是否已关闭
            if channel.eof() {
                debug!("SSH 通道已关闭");
                break;
            }
        }

        // 等待读取线程结束
        let _ = read_handle.join();

        // 关闭通道
        channel.close().ok();
        channel.wait_close().ok();

        println!("\n\n=== Shell 会话已结束 ===");

        Ok(())
    }
    
    /// 执行单个命令（非交互式）
    pub fn exec_command(&self, command: &str) -> Result<()> {
        println!("执行命令: {}", command);
        let output = self.ssh_client.exec_command(command)?;
        print!("{}", output);
        Ok(())
    }
}

/// 将按键事件转换为字节
#[allow(dead_code)]
fn key_to_bytes(key_event: &KeyEvent) -> Option<Vec<u8>> {
    match key_event.code {
        KeyCode::Char(c) => {
            if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+字母组合
                match c {
                    'a'..='z' => Some(vec![(c as u8) - b'a' + 1]),
                    'A'..='Z' => Some(vec![(c as u8) - b'A' + 1]),
                    '@' => Some(vec![0x00]), // Ctrl+@
                    '[' => Some(vec![0x1b]), // Ctrl+[
                    '\\' => Some(vec![0x1c]), // Ctrl+\
                    ']' => Some(vec![0x1d]), // Ctrl+]
                    '^' => Some(vec![0x1e]), // Ctrl+^
                    '_' => Some(vec![0x1f]), // Ctrl+_
                    '?' => Some(vec![0x7f]), // Ctrl+?
                    _ => None,
                }
            } else if key_event.modifiers.contains(KeyModifiers::ALT) {
                // Alt+字符组合
                let mut bytes = vec![0x1b];
                bytes.extend(c.to_string().into_bytes());
                Some(bytes)
            } else {
                // 普通字符
                Some(c.to_string().into_bytes())
            }
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(vec![0x1b, b'[', b'A']),
        KeyCode::Down => Some(vec![0x1b, b'[', b'B']),
        KeyCode::Right => Some(vec![0x1b, b'[', b'C']),
        KeyCode::Left => Some(vec![0x1b, b'[', b'D']),
        KeyCode::Home => Some(vec![0x1b, b'[', b'H']),
        KeyCode::End => Some(vec![0x1b, b'[', b'F']),
        KeyCode::PageUp => Some(vec![0x1b, b'[', b'5', b'~']),
        KeyCode::PageDown => Some(vec![0x1b, b'[', b'6', b'~']),
        KeyCode::Delete => Some(vec![0x1b, b'[', b'3', b'~']),
        KeyCode::Insert => Some(vec![0x1b, b'[', b'2', b'~']),
        KeyCode::F(n) => {
            // F1-F12 功能键
            match n {
                1 => Some(vec![0x1b, b'O', b'P']),
                2 => Some(vec![0x1b, b'O', b'Q']),
                3 => Some(vec![0x1b, b'O', b'R']),
                4 => Some(vec![0x1b, b'O', b'S']),
                5 => Some(vec![0x1b, b'[', b'1', b'5', b'~']),
                6 => Some(vec![0x1b, b'[', b'1', b'7', b'~']),
                7 => Some(vec![0x1b, b'[', b'1', b'8', b'~']),
                8 => Some(vec![0x1b, b'[', b'1', b'9', b'~']),
                9 => Some(vec![0x1b, b'[', b'2', b'0', b'~']),
                10 => Some(vec![0x1b, b'[', b'2', b'1', b'~']),
                11 => Some(vec![0x1b, b'[', b'2', b'3', b'~']),
                12 => Some(vec![0x1b, b'[', b'2', b'4', b'~']),
                _ => None,
            }
        }
        _ => None,
    }
}

/// 简单的命令行 shell（非原始模式）
pub struct SimpleShell<'a> {
    ssh_client: &'a SshClient,
}

impl<'a> SimpleShell<'a> {
    pub fn new(ssh_client: &'a SshClient) -> Self {
        Self { ssh_client }
    }
    
    /// 启动简单的命令行界面
    pub fn start(&self) -> Result<()> {
        println!("=== SSH 命令行模式 ===");
        println!("连接到: {}@{}", 
            self.ssh_client.config().username, 
            self.ssh_client.config().host);
        println!("输入命令并按回车执行，输入 'exit' 或 'quit' 退出");
        println!("=====================\n");
        
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        
        loop {
            print!("{}@{}> ", 
                self.ssh_client.config().username,
                self.ssh_client.config().host);
            stdout.flush()?;
            
            let mut input = String::new();
            stdin.read_line(&mut input)?;
            
            let command = input.trim();
            
            if command.is_empty() {
                continue;
            }
            
            if command == "exit" || command == "quit" {
                break;
            }
            
            match self.ssh_client.exec_command(command) {
                Ok(output) => print!("{}", output),
                Err(e) => eprintln!("错误: {}", e),
            }
        }
        
        println!("\n再见！");
        Ok(())
    }
}

/// 过滤控制序列，移除 CPR (Cursor Position Report) 等不需要的序列
fn filter_control_sequences(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        // 检查是否是 ESC 序列的开始
        if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'[' {
            // 找到 CSI 序列的结束
            let start = i;
            let mut j = i + 2;

            // 读取参数和中间字节，直到找到结束字符
            while j < data.len() {
                let c = data[j];

                // CSI 序列的结束字符 (0x40-0x7E)
                if c >= 0x40 && c <= 0x7E {
                    // 检查是否是 CPR (Cursor Position Report): ESC[n;mR
                    if c == b'R' {
                        let params = &data[i+2..j];
                        // 如果参数只包含数字和分号，这是 CPR，过滤掉
                        if params.iter().all(|&b| b.is_ascii_digit() || b == b';') {
                            // 跳过整个 CPR 序列
                            i = j + 1;
                            break;
                        }
                    }

                    // 不是 CPR，保留整个序列
                    result.extend_from_slice(&data[start..=j]);
                    i = j + 1;
                    break;
                }

                j += 1;
            }

            // 如果没有找到结束字符，保留原始数据
            if j >= data.len() {
                result.extend_from_slice(&data[start..]);
                break;
            }
        } else {
            // 普通字符，直接添加
            result.push(data[i]);
            i += 1;
        }
    }

    result
}

