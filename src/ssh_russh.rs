use anyhow::{Context, Result, anyhow};
use log::{debug, info};
use russh::*;
use russh_keys::*;
use std::sync::Arc;

/// SSH 认证方法
#[derive(Debug, Clone)]
pub enum AuthMethod {
    Password(String),
    PublicKey(String),
}

/// SSH 连接配置
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
}

impl SshConfig {
    pub fn new(host: String, port: u16, username: String, auth: AuthMethod) -> Self {
        Self {
            host,
            port,
            username,
            auth,
        }
    }
}

/// Russh 客户端处理器
pub struct ClientHandler;

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // 在生产环境中应该验证服务器密钥
        // 这里为了简单起见，接受所有密钥
        Ok(true)
    }
}

/// Russh SSH 客户端
pub struct RusshClient {
    config: SshConfig,
    session: Option<client::Handle<ClientHandler>>,
}

impl RusshClient {
    /// 创建新的 SSH 客户端
    pub fn new(config: SshConfig) -> Self {
        Self {
            config,
            session: None,
        }
    }

    /// 连接到 SSH 服务器
    pub async fn connect(&mut self) -> Result<()> {
        info!("正在连接到 {}:{}",  self.config.host, self.config.port);

        // 创建 SSH 客户端配置
        let client_config = client::Config::default();
        let sh = ClientHandler;

        // 连接到服务器
        let mut session = client::connect(
            Arc::new(client_config),
            (self.config.host.as_str(), self.config.port),
            sh,
        )
        .await
        .context("无法连接到 SSH 服务器")?;

        // 认证
        let auth_result = match &self.config.auth {
            AuthMethod::Password(password) => {
                debug!("使用密码认证");
                session
                    .authenticate_password(self.config.username.clone(), password.clone())
                    .await
            }
            AuthMethod::PublicKey(key_path) => {
                debug!("使用公钥认证: {}", key_path);
                let key_pair = load_secret_key(key_path, None)
                    .context("无法加载私钥")?;
                session
                    .authenticate_publickey(self.config.username.clone(), Arc::new(key_pair))
                    .await
            }
        };

        if !auth_result.context("认证失败")? {
            return Err(anyhow!("认证被拒绝"));
        }

        info!("SSH 连接成功");
        self.session = Some(session);
        Ok(())
    }

    /// 获取会话引用
    pub fn session(&mut self) -> Result<&mut client::Handle<ClientHandler>> {
        self.session.as_mut().ok_or_else(|| anyhow!("未连接"))
    }

    /// 获取配置
    pub fn config(&self) -> &SshConfig {
        &self.config
    }

    /// 断开连接
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(session) = self.session.take() {
            session
                .disconnect(Disconnect::ByApplication, "", "English")
                .await
                .context("断开连接失败")?;
        }
        Ok(())
    }
}

impl Drop for RusshClient {
    fn drop(&mut self) {
        // 注意：这里不能调用异步方法
        // 实际断开会在 session drop 时自动处理
    }
}

