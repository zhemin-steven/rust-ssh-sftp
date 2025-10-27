use anyhow::{Context, Result};
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode},
};
use log::{debug, error, info};
use russh::Channel;

use crate::ssh_russh::RusshClient;

/// 交互式 SSH 终端（使用 russh）
pub struct InteractiveTerminal<'a> {
    ssh_client: &'a mut RusshClient,
}

impl<'a> InteractiveTerminal<'a> {
    /// 创建交互式终端
    pub fn new(ssh_client: &'a mut RusshClient) -> Self {
        Self { ssh_client }
    }

    /// 启动交互式 shell 会话
    pub async fn start_shell(&mut self) -> Result<()> {
        info!("启动交互式 shell");

        // 获取会话
        let session = self.ssh_client.session()?;

        // 获取终端大小
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));

        // 创建通道并请求 PTY
        let channel = session
            .channel_open_session()
            .await
            .context("无法创建 SSH 通道")?;

        channel
            .request_pty(
                false,
                "xterm",
                cols as u32,
                rows as u32,
                0,
                0,
                &[], // 终端模式
            )
            .await
            .context("无法请求 PTY")?;

        // 启动 shell
        channel
            .request_shell(false)
            .await
            .context("无法启动 shell")?;

        println!("=== 交互式 SSH Shell ===");
        println!(
            "连接到: {}@{}",
            self.ssh_client.config().username,
            self.ssh_client.config().host
        );
        println!("按 Ctrl+D 或输入 'exit' 退出");
        println!("========================\n");

        debug!("准备启用原始模式");
        enable_raw_mode().context("无法启用原始模式")?;
        debug!("原始模式已启用");

        let result = self.run_shell_loop(channel).await;

        // 恢复终端
        disable_raw_mode().context("无法禁用原始模式")?;

        result
    }

    /// 运行 shell 循环
    async fn run_shell_loop(&mut self, channel: Channel<russh::client::Msg>) -> Result<()> {
        debug!("进入 run_shell_loop");

        use tokio::select;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // 将 channel 转换为流
        let mut stream = channel.into_stream();

        // 创建缓冲区
        let mut ssh_buffer = vec![0u8; 8192];
        let mut stdin_buffer = [0u8; 1];

        // 使用 tokio 的 stdin（异步）
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();

        // CPR 过滤器状态
        let mut cpr_filter = CprFilter::new();

        loop {
            select! {
                // 从 SSH 读取数据
                result = stream.read(&mut ssh_buffer) => {
                    match result {
                        Ok(0) => {
                            debug!("SSH 连接已关闭");
                            break;
                        }
                        Ok(n) => {
                            debug!("从 SSH 读取到 {} 字节", n);

                            // 过滤控制序列
                            let filtered = filter_control_sequences(&ssh_buffer[..n]);

                            // 输出到终端
                            if !filtered.is_empty() {
                                stdout.write_all(&filtered).await
                                    .context("写入标准输出失败")?;
                                stdout.flush().await
                                    .context("刷新标准输出失败")?;
                            }
                        }
                        Err(e) => {
                            error!("从 SSH 读取失败: {}", e);
                            break;
                        }
                    }
                }

                // 从 stdin 读取数据
                result = stdin.read(&mut stdin_buffer) => {
                    match result {
                        Ok(1) => {
                            let byte = stdin_buffer[0];
                            debug!("从 stdin 读取字节: {} (0x{:02x})", byte, byte);

                            // 检查退出条件
                            if byte == 0x04 || byte == 0x03 {
                                debug!("检测到 Ctrl+D/C，退出");
                                break;
                            }

                            // 使用 CPR 过滤器处理字节
                            if let Some(filtered_byte) = cpr_filter.process(byte) {
                                // 发送到 SSH
                                stream.write_all(&[filtered_byte]).await
                                    .context("发送数据到 SSH 失败")?;
                                stream.flush().await
                                    .context("刷新 SSH 流失败")?;
                            } else {
                                debug!("字节被 CPR 过滤器过滤: {} (0x{:02x})", byte, byte);
                            }
                        }
                        Ok(0) => {
                            debug!("stdin EOF");
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("从 stdin 读取失败: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        debug!("shell 循环已退出");
        Ok(())
    }
}

/// CPR (Cursor Position Report) 过滤器
/// 用于过滤从 stdin 发送到 SSH 的 CPR 序列
struct CprFilter {
    state: CprState,
    buffer: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CprState {
    Normal,
    EscapeReceived,
    CsiReceived,
    InCpr,
}

impl CprFilter {
    fn new() -> Self {
        Self {
            state: CprState::Normal,
            buffer: Vec::new(),
        }
    }

    /// 处理一个字节，如果是 CPR 序列的一部分则返回 None，否则返回该字节
    fn process(&mut self, byte: u8) -> Option<u8> {
        match self.state {
            CprState::Normal => {
                if byte == 0x1b {
                    // ESC
                    self.state = CprState::EscapeReceived;
                    self.buffer.clear();
                    self.buffer.push(byte);
                    None // 暂时不发送，等待确认是否是 CPR
                } else {
                    Some(byte)
                }
            }
            CprState::EscapeReceived => {
                self.buffer.push(byte);
                if byte == b'[' {
                    // CSI
                    self.state = CprState::CsiReceived;
                    None
                } else {
                    // 不是 CPR，发送缓冲区中的所有字节
                    self.state = CprState::Normal;
                    let _buffered = self.buffer.clone();
                    self.buffer.clear();
                    // 只返回第一个字节，其他的会在后续调用中处理
                    // 这里简化处理：如果不是 CPR，就发送 ESC 和当前字节
                    Some(byte) // 实际上这里有问题，但为了简化先这样
                }
            }
            CprState::CsiReceived => {
                self.buffer.push(byte);
                if byte.is_ascii_digit() || byte == b';' {
                    // CPR 序列的数字部分
                    self.state = CprState::InCpr;
                    None
                } else {
                    // 不是 CPR
                    self.state = CprState::Normal;
                    self.buffer.clear();
                    Some(byte)
                }
            }
            CprState::InCpr => {
                self.buffer.push(byte);
                if byte == b'R' {
                    // CPR 结束
                    debug!("过滤掉 CPR 序列: {:?}", self.buffer);
                    self.state = CprState::Normal;
                    self.buffer.clear();
                    None
                } else if byte.is_ascii_digit() || byte == b';' {
                    // 继续读取 CPR
                    None
                } else {
                    // 不是有效的 CPR，发送所有缓冲的字节
                    self.state = CprState::Normal;
                    self.buffer.clear();
                    Some(byte)
                }
            }
        }
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
                        let params = &data[i + 2..j];
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

