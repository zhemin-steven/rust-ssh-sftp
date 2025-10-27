use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use ssh2::Sftp;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::ssh::SshClient;

/// SFTP 客户端
pub struct SftpClient<'a> {
    sftp: Sftp,
    #[allow(dead_code)]
    ssh_client: &'a SshClient,
}

/// 文件信息
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    #[allow(dead_code)]
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    #[allow(dead_code)]
    pub permissions: u32,
}

impl<'a> SftpClient<'a> {
    /// 创建 SFTP 客户端
    pub fn new(ssh_client: &'a SshClient) -> Result<Self> {
        info!("初始化 SFTP 会话");
        let sftp = ssh_client.session().sftp()
            .context("无法创建 SFTP 会话")?;
        
        Ok(Self { sftp, ssh_client })
    }
    
    /// 列出目录内容
    pub fn list_dir(&self, remote_path: &str) -> Result<Vec<FileInfo>> {
        debug!("列出目录: {}", remote_path);
        
        let path = Path::new(remote_path);
        let entries = self.sftp.readdir(path)
            .context(format!("无法读取目录: {}", remote_path))?;
        
        let mut files = Vec::new();
        
        for (path, stat) in entries {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            files.push(FileInfo {
                name: name.clone(),
                path: path.to_string_lossy().to_string(),
                size: stat.size.unwrap_or(0),
                is_dir: stat.is_dir(),
                permissions: stat.perm.unwrap_or(0),
            });
        }
        
        // 按名称排序，目录在前
        files.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        Ok(files)
    }
    
    /// 上传文件
    pub fn upload_file(&self, local_path: &str, remote_path: &str, show_progress: bool) -> Result<()> {
        info!("上传文件: {} -> {}", local_path, remote_path);
        
        let local = Path::new(local_path);
        let remote = Path::new(remote_path);
        
        // 打开本地文件
        let mut local_file = File::open(local)
            .context(format!("无法打开本地文件: {}", local_path))?;
        
        // 获取文件大小
        let file_size = local_file.metadata()?.len();
        
        // 创建远程文件
        let mut remote_file = self.sftp.create(remote)
            .context(format!("无法创建远程文件: {}", remote_path))?;
        
        // 创建进度条
        let pb = if show_progress {
            let pb = ProgressBar::new(file_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            pb.set_message(format!("上传: {}", local_path));
            Some(pb)
        } else {
            None
        };
        
        // 传输文件
        let mut buffer = vec![0u8; 8192];
        let mut transferred = 0u64;
        
        loop {
            let n = local_file.read(&mut buffer)
                .context("读取本地文件失败")?;
            
            if n == 0 {
                break;
            }
            
            remote_file.write_all(&buffer[..n])
                .context("写入远程文件失败")?;
            
            transferred += n as u64;
            
            if let Some(ref pb) = pb {
                pb.set_position(transferred);
            }
        }
        
        if let Some(pb) = pb {
            pb.finish_with_message(format!("上传完成: {}", local_path));
        }
        
        info!("文件上传成功: {} ({} 字节)", remote_path, transferred);
        Ok(())
    }
    
    /// 下载文件
    pub fn download_file(&self, remote_path: &str, local_path: &str, show_progress: bool) -> Result<()> {
        info!("下载文件: {} -> {}", remote_path, local_path);
        
        let remote = Path::new(remote_path);
        let local = Path::new(local_path);
        
        // 打开远程文件
        let mut remote_file = self.sftp.open(remote)
            .context(format!("无法打开远程文件: {}", remote_path))?;
        
        // 获取文件大小
        let file_size = remote_file.stat()?.size.unwrap_or(0);
        
        // 创建本地文件
        let mut local_file = File::create(local)
            .context(format!("无法创建本地文件: {}", local_path))?;
        
        // 创建进度条
        let pb = if show_progress {
            let pb = ProgressBar::new(file_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            pb.set_message(format!("下载: {}", remote_path));
            Some(pb)
        } else {
            None
        };
        
        // 传输文件
        let mut buffer = vec![0u8; 8192];
        let mut transferred = 0u64;
        
        loop {
            let n = remote_file.read(&mut buffer)
                .context("读取远程文件失败")?;
            
            if n == 0 {
                break;
            }
            
            local_file.write_all(&buffer[..n])
                .context("写入本地文件失败")?;
            
            transferred += n as u64;
            
            if let Some(ref pb) = pb {
                pb.set_position(transferred);
            }
        }
        
        if let Some(pb) = pb {
            pb.finish_with_message(format!("下载完成: {}", local_path));
        }
        
        info!("文件下载成功: {} ({} 字节)", local_path, transferred);
        Ok(())
    }
    
    /// 创建目录
    pub fn mkdir(&self, remote_path: &str) -> Result<()> {
        info!("创建目录: {}", remote_path);
        self.sftp.mkdir(Path::new(remote_path), 0o755)
            .context(format!("无法创建目录: {}", remote_path))?;
        Ok(())
    }
    
    /// 删除文件
    pub fn remove_file(&self, remote_path: &str) -> Result<()> {
        info!("删除文件: {}", remote_path);
        self.sftp.unlink(Path::new(remote_path))
            .context(format!("无法删除文件: {}", remote_path))?;
        Ok(())
    }
    
    /// 删除目录
    #[allow(dead_code)]
    pub fn remove_dir(&self, remote_path: &str) -> Result<()> {
        info!("删除目录: {}", remote_path);
        self.sftp.rmdir(Path::new(remote_path))
            .context(format!("无法删除目录: {}", remote_path))?;
        Ok(())
    }
    
    /// 重命名文件或目录
    #[allow(dead_code)]
    pub fn rename(&self, old_path: &str, new_path: &str) -> Result<()> {
        info!("重命名: {} -> {}", old_path, new_path);
        self.sftp.rename(Path::new(old_path), Path::new(new_path), None)
            .context(format!("无法重命名: {} -> {}", old_path, new_path))?;
        Ok(())
    }
    
    /// 获取文件信息
    #[allow(dead_code)]
    pub fn stat(&self, remote_path: &str) -> Result<FileInfo> {
        let path = Path::new(remote_path);
        let stat = self.sftp.stat(path)
            .context(format!("无法获取文件信息: {}", remote_path))?;
        
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        Ok(FileInfo {
            name,
            path: remote_path.to_string(),
            size: stat.size.unwrap_or(0),
            is_dir: stat.is_dir(),
            permissions: stat.perm.unwrap_or(0),
        })
    }
}

