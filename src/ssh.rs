use anyhow::{Context, Result};
use ssh2::Session;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use log::{info, debug, error};

/// SSH 认证方式
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// 密码认证
    Password(String),
    /// 公钥认证
    PublicKey {
        public_key: Option<String>,
        private_key: String,
        passphrase: Option<String>,
    },
}

/// SSH 连接配置
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
}

/// SSH 客户端
pub struct SshClient {
    session: Session,
    config: SshConfig,
}

impl SshClient {
    /// 创建新的 SSH 连接
    pub fn connect(config: SshConfig) -> Result<Self> {
        info!("正在连接到 {}@{}:{}", config.username, config.host, config.port);
        
        // 建立 TCP 连接
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))
            .context("无法建立 TCP 连接")?;
        
        // 创建 SSH 会话
        let mut session = Session::new().context("无法创建 SSH 会话")?;
        session.set_tcp_stream(tcp);
        session.handshake().context("SSH 握手失败")?;
        
        // 认证
        match &config.auth {
            AuthMethod::Password(password) => {
                debug!("使用密码认证");
                session
                    .userauth_password(&config.username, password)
                    .context("密码认证失败")?;
            }
            AuthMethod::PublicKey {
                public_key,
                private_key,
                passphrase,
            } => {
                debug!("使用公钥认证");
                session
                    .userauth_pubkey_file(
                        &config.username,
                        public_key.as_deref().map(Path::new),
                        Path::new(private_key),
                        passphrase.as_deref(),
                    )
                    .context("公钥认证失败")?;
            }
        }
        
        if !session.authenticated() {
            anyhow::bail!("认证失败");
        }
        
        info!("SSH 连接成功");
        
        Ok(Self { session, config })
    }
    
    /// 执行单个命令
    pub fn exec_command(&self, command: &str) -> Result<String> {
        debug!("执行命令: {}", command);
        
        let mut channel = self.session.channel_session()
            .context("无法创建通道")?;
        
        channel.exec(command)
            .context("命令执行失败")?;
        
        let mut output = String::new();
        channel.read_to_string(&mut output)
            .context("读取输出失败")?;
        
        channel.wait_close()
            .context("等待通道关闭失败")?;
        
        let exit_status = channel.exit_status()
            .context("获取退出状态失败")?;
        
        if exit_status != 0 {
            let mut stderr = String::new();
            channel.stderr().read_to_string(&mut stderr).ok();
            error!("命令执行失败，退出码: {}, 错误: {}", exit_status, stderr);
        }
        
        Ok(output)
    }
    
    /// 获取 SSH 会话引用（用于 SFTP）
    pub fn session(&self) -> &Session {
        &self.session
    }
    
    /// 获取配置信息
    pub fn config(&self) -> &SshConfig {
        &self.config
    }
    
    /// 测试连接是否有效
    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.session.authenticated()
    }
}

impl Drop for SshClient {
    fn drop(&mut self) {
        info!("断开 SSH 连接");
        let _ = self.session.disconnect(None, "客户端断开连接", None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ssh_config_creation() {
        let config = SshConfig {
            host: "example.com".to_string(),
            port: 22,
            username: "user".to_string(),
            auth: AuthMethod::Password("password".to_string()),
        };
        
        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 22);
    }
}

