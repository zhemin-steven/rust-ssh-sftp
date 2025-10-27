use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::crypto::CryptoManager;
use crate::ssh::{AuthMethod, SshConfig};

/// 保存的连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub auth_type: String, // "password" 或 "publickey"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_path: Option<String>,
    /// 加密的密码（仅用于密码认证）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_password: Option<String>,
    /// 加密的私钥密码（仅用于公钥认证）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_passphrase: Option<String>,
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub connections: HashMap<String, SavedConnection>,
    #[serde(default)]
    pub default_connection: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            connections: HashMap::new(),
            default_connection: None,
        }
    }
}

impl AppConfig {
    /// 获取配置文件路径
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("rust-ssh-sftp");
        
        // 确保配置目录存在
        fs::create_dir_all(&config_dir)
            .context("无法创建配置目录")?;
        
        Ok(config_dir.join("config.toml"))
    }
    
    /// 从文件加载配置
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if !config_path.exists() {
            return Ok(Self::default());
        }
        
        let content = fs::read_to_string(&config_path)
            .context("无法读取配置文件")?;
        
        let config: AppConfig = toml::from_str(&content)
            .context("无法解析配置文件")?;
        
        Ok(config)
    }
    
    /// 保存配置到文件
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        
        let content = toml::to_string_pretty(self)
            .context("无法序列化配置")?;
        
        fs::write(&config_path, content)
            .context("无法写入配置文件")?;
        
        Ok(())
    }
    
    /// 添加连接配置
    pub fn add_connection(&mut self, connection: SavedConnection) {
        let name = connection.name.clone();
        self.connections.insert(name.clone(), connection);
        
        // 如果是第一个连接，设为默认
        if self.default_connection.is_none() {
            self.default_connection = Some(name);
        }
    }
    
    /// 删除连接配置
    pub fn remove_connection(&mut self, name: &str) -> Result<()> {
        self.connections.remove(name)
            .context(format!("连接 '{}' 不存在", name))?;
        
        // 如果删除的是默认连接，清除默认设置
        if self.default_connection.as_deref() == Some(name) {
            self.default_connection = None;
        }
        
        Ok(())
    }
    
    /// 获取连接配置
    pub fn get_connection(&self, name: &str) -> Option<&SavedConnection> {
        self.connections.get(name)
    }
    
    /// 列出所有连接
    pub fn list_connections(&self) -> Vec<&SavedConnection> {
        let mut connections: Vec<_> = self.connections.values().collect();
        connections.sort_by(|a, b| a.name.cmp(&b.name));
        connections
    }
    
    /// 设置默认连接
    pub fn set_default(&mut self, name: &str) -> Result<()> {
        if !self.connections.contains_key(name) {
            anyhow::bail!("连接 '{}' 不存在", name);
        }
        self.default_connection = Some(name.to_string());
        Ok(())
    }
    
    /// 获取默认连接
    #[allow(dead_code)]
    pub fn get_default_connection(&self) -> Option<&SavedConnection> {
        self.default_connection.as_ref()
            .and_then(|name| self.connections.get(name))
    }
}

impl SavedConnection {
    /// 转换为 SshConfig（需要密码或密钥密码）
    pub fn to_ssh_config(&self, password: Option<String>, passphrase: Option<String>) -> Result<SshConfig> {
        let auth = match self.auth_type.as_str() {
            "password" => {
                let pwd = password.context("密码认证需要提供密码")?;
                AuthMethod::Password(pwd)
            }
            "publickey" => {
                let private_key = self.private_key_path.clone()
                    .context("公钥认证需要提供私钥路径")?;

                AuthMethod::PublicKey {
                    public_key: self.public_key_path.clone(),
                    private_key,
                    passphrase,
                }
            }
            _ => anyhow::bail!("未知的认证类型: {}", self.auth_type),
        };

        Ok(SshConfig {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            auth,
        })
    }

    /// 转换为 SshConfig（自动解密保存的密码）
    pub fn to_ssh_config_with_decryption(
        &self,
        crypto: &CryptoManager,
        password_override: Option<String>,
        passphrase_override: Option<String>,
    ) -> Result<SshConfig> {
        let auth = match self.auth_type.as_str() {
            "password" => {
                let pwd = if let Some(pwd) = password_override {
                    pwd
                } else if let Some(encrypted) = &self.encrypted_password {
                    crypto.decrypt(encrypted)
                        .context("解密密码失败（可能是主密码错误）")?
                } else {
                    anyhow::bail!("未保存密码，请手动输入");
                };
                AuthMethod::Password(pwd)
            }
            "publickey" => {
                let private_key = self.private_key_path.clone()
                    .context("公钥认证需要提供私钥路径")?;

                let passphrase = if let Some(pp) = passphrase_override {
                    Some(pp)
                } else if let Some(encrypted) = &self.encrypted_passphrase {
                    Some(crypto.decrypt(encrypted)
                        .context("解密私钥密码失败（可能是主密码错误）")?)
                } else {
                    None
                };

                AuthMethod::PublicKey {
                    public_key: self.public_key_path.clone(),
                    private_key,
                    passphrase,
                }
            }
            _ => anyhow::bail!("未知的认证类型: {}", self.auth_type),
        };

        Ok(SshConfig {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            auth,
        })
    }

    /// 检查是否保存了密码
    pub fn has_saved_password(&self) -> bool {
        match self.auth_type.as_str() {
            "password" => self.encrypted_password.is_some(),
            "publickey" => self.encrypted_passphrase.is_some(),
            _ => false,
        }
    }
    
    /// 创建新的密码认证连接
    pub fn new_password(name: String, host: String, port: u16, username: String) -> Self {
        Self {
            name,
            host,
            port,
            username,
            auth_type: "password".to_string(),
            private_key_path: None,
            public_key_path: None,
            encrypted_password: None,
            encrypted_passphrase: None,
        }
    }

    /// 创建新的密码认证连接（带加密密码）
    pub fn new_password_with_encrypted(
        name: String,
        host: String,
        port: u16,
        username: String,
        encrypted_password: String,
    ) -> Self {
        Self {
            name,
            host,
            port,
            username,
            auth_type: "password".to_string(),
            private_key_path: None,
            public_key_path: None,
            encrypted_password: Some(encrypted_password),
            encrypted_passphrase: None,
        }
    }

    /// 创建新的公钥认证连接
    pub fn new_publickey(
        name: String,
        host: String,
        port: u16,
        username: String,
        private_key_path: String,
        public_key_path: Option<String>,
    ) -> Self {
        Self {
            name,
            host,
            port,
            username,
            auth_type: "publickey".to_string(),
            private_key_path: Some(private_key_path),
            public_key_path,
            encrypted_password: None,
            encrypted_passphrase: None,
        }
    }

    /// 创建新的公钥认证连接（带加密的私钥密码）
    #[allow(dead_code)]
    pub fn new_publickey_with_encrypted(
        name: String,
        host: String,
        port: u16,
        username: String,
        private_key_path: String,
        public_key_path: Option<String>,
        encrypted_passphrase: String,
    ) -> Self {
        Self {
            name,
            host,
            port,
            username,
            auth_type: "publickey".to_string(),
            private_key_path: Some(private_key_path),
            public_key_path,
            encrypted_password: None,
            encrypted_passphrase: Some(encrypted_passphrase),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_saved_connection_creation() {
        let conn = SavedConnection::new_password(
            "test".to_string(),
            "example.com".to_string(),
            22,
            "user".to_string(),
        );
        
        assert_eq!(conn.name, "test");
        assert_eq!(conn.auth_type, "password");
    }
    
    #[test]
    fn test_app_config_operations() {
        let mut config = AppConfig::default();
        
        let conn = SavedConnection::new_password(
            "test".to_string(),
            "example.com".to_string(),
            22,
            "user".to_string(),
        );
        
        config.add_connection(conn);
        assert_eq!(config.connections.len(), 1);
        assert!(config.get_connection("test").is_some());
    }
}

