use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;
use std::fs;
use std::path::PathBuf;

/// 加密密钥管理器
pub struct CryptoManager {
    master_key: [u8; 32],
}

impl CryptoManager {
    /// 创建新的加密管理器
    /// 使用主密码派生加密密钥
    pub fn new(master_password: &str) -> Result<Self> {
        let master_key = Self::derive_key(master_password)?;
        Ok(Self { master_key })
    }

    /// 从主密码派生加密密钥
    fn derive_key(password: &str) -> Result<[u8; 32]> {
        // 获取或创建盐值
        let salt = Self::get_or_create_salt()?;
        
        // 使用 Argon2 派生密钥
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("密钥派生失败: {}", e))?;
        
        // 提取密钥
        let hash = password_hash.hash.context("无法获取哈希值")?;
        let hash_bytes = hash.as_bytes();
        
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash_bytes[..32]);
        
        Ok(key)
    }

    /// 获取或创建盐值
    fn get_or_create_salt() -> Result<SaltString> {
        let salt_path = Self::salt_path()?;
        
        if salt_path.exists() {
            // 读取现有盐值
            let salt_str = fs::read_to_string(&salt_path)
                .context("无法读取盐值文件")?;
            SaltString::from_b64(&salt_str.trim())
                .map_err(|e| anyhow::anyhow!("无效的盐值: {}", e))
        } else {
            // 创建新盐值
            let salt = SaltString::generate(&mut OsRng);
            
            // 保存盐值
            if let Some(parent) = salt_path.parent() {
                fs::create_dir_all(parent)
                    .context("无法创建配置目录")?;
            }
            
            fs::write(&salt_path, salt.as_str())
                .context("无法保存盐值")?;
            
            Ok(salt)
        }
    }

    /// 获取盐值文件路径
    fn salt_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("rust-ssh-sftp");
        
        Ok(config_dir.join(".salt"))
    }

    /// 加密字符串
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        // 创建加密器
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| anyhow::anyhow!("创建加密器失败: {}", e))?;
        
        // 生成随机 nonce（12 字节）
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // 加密
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("加密失败: {}", e))?;
        
        // 组合 nonce 和 ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);
        
        // Base64 编码
        Ok(general_purpose::STANDARD.encode(&result))
    }

    /// 解密字符串
    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        // Base64 解码
        let data = general_purpose::STANDARD
            .decode(encrypted)
            .context("Base64 解码失败")?;
        
        if data.len() < 12 {
            anyhow::bail!("加密数据太短");
        }
        
        // 分离 nonce 和 ciphertext
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        // 创建解密器
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| anyhow::anyhow!("创建解密器失败: {}", e))?;
        
        // 解密
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("解密失败（可能是主密码错误）: {}", e))?;
        
        // 转换为字符串
        String::from_utf8(plaintext)
            .context("解密后的数据不是有效的 UTF-8")
    }

    /// 获取或创建主密码
    /// 如果是首次使用，会提示用户设置主密码
    /// 如果已有主密码，会提示用户输入
    pub fn get_master_password(is_first_time: bool) -> Result<String> {
        if is_first_time {
            println!("首次使用加密功能，请设置主密码（用于加密保存的密码）");
            println!("⚠️  请牢记此密码，忘记后无法恢复已保存的密码！");
            
            let password = rpassword::prompt_password("请输入主密码: ")
                .context("无法读取密码")?;
            
            if password.is_empty() {
                anyhow::bail!("主密码不能为空");
            }
            
            let confirm = rpassword::prompt_password("请再次输入主密码: ")
                .context("无法读取密码")?;
            
            if password != confirm {
                anyhow::bail!("两次输入的密码不一致");
            }
            
            Ok(password)
        } else {
            let password = rpassword::prompt_password("请输入主密码: ")
                .context("无法读取密码")?;
            
            if password.is_empty() {
                anyhow::bail!("主密码不能为空");
            }
            
            Ok(password)
        }
    }

    /// 检查是否已设置主密码（通过检查盐值文件是否存在）
    pub fn has_master_password() -> bool {
        Self::salt_path()
            .map(|path| path.exists())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let manager = CryptoManager::new("test_password_123").unwrap();
        
        let plaintext = "my_secret_password";
        let encrypted = manager.encrypt(plaintext).unwrap();
        
        // 加密后应该不同
        assert_ne!(encrypted, plaintext);
        
        // 解密后应该相同
        let decrypted = manager.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_password() {
        let manager1 = CryptoManager::new("password1").unwrap();
        let encrypted = manager1.encrypt("secret").unwrap();
        
        let manager2 = CryptoManager::new("password2").unwrap();
        let result = manager2.decrypt(&encrypted);
        
        // 使用错误的密码应该解密失败
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_encryptions() {
        let manager = CryptoManager::new("test_password").unwrap();
        
        let plaintext = "test_data";
        let encrypted1 = manager.encrypt(plaintext).unwrap();
        let encrypted2 = manager.encrypt(plaintext).unwrap();
        
        // 每次加密结果应该不同（因为使用随机 nonce）
        assert_ne!(encrypted1, encrypted2);
        
        // 但都应该能正确解密
        assert_eq!(manager.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(manager.decrypt(&encrypted2).unwrap(), plaintext);
    }
}

