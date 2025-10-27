use clap::{Parser, Subcommand};

/// Rust SSH/SFTP 客户端 - 类似 FinalShell 的跨平台终端工具
#[derive(Parser, Debug)]
#[command(name = "rust-ssh-sftp")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 连接到 SSH 服务器
    Connect {
        /// 连接名称（从配置中）或 user@host 格式。如果不提供，将显示交互式选择菜单
        target: Option<String>,

        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,

        /// 使用交互式 shell（原始模式）
        #[arg(short = 'I', long)]
        interactive: bool,

        /// 私钥文件路径（用于公钥认证）
        #[arg(short = 'i', long)]
        identity_file: Option<String>,

        /// 保存密码（加密保存到配置文件）
        #[arg(long)]
        save_password: bool,

        /// 保存为新的连接配置
        #[arg(long)]
        save_as: Option<String>,
    },
    
    /// 执行远程命令
    Exec {
        /// 连接名称或 user@host 格式
        target: String,
        
        /// 要执行的命令
        command: String,
        
        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 私钥文件路径
        #[arg(short = 'i', long)]
        identity_file: Option<String>,
    },
    
    /// SFTP 文件传输
    Sftp {
        #[command(subcommand)]
        action: SftpCommands,
    },
    
    /// 管理保存的连接配置
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },

    /// 启动图形界面
    Gui,
}

#[derive(Subcommand, Debug)]
pub enum SftpCommands {
    /// 上传文件
    Upload {
        /// 连接名称或 user@host 格式
        target: String,
        
        /// 本地文件路径
        local_path: String,
        
        /// 远程文件路径
        remote_path: String,
        
        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 私钥文件路径
        #[arg(short = 'i', long)]
        identity_file: Option<String>,
        
        /// 不显示进度条
        #[arg(long)]
        no_progress: bool,
    },
    
    /// 下载文件
    Download {
        /// 连接名称或 user@host 格式
        target: String,
        
        /// 远程文件路径
        remote_path: String,
        
        /// 本地文件路径
        local_path: String,
        
        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 私钥文件路径
        #[arg(short = 'i', long)]
        identity_file: Option<String>,
        
        /// 不显示进度条
        #[arg(long)]
        no_progress: bool,
    },
    
    /// 列出远程目录
    List {
        /// 连接名称或 user@host 格式
        target: String,
        
        /// 远程目录路径
        remote_path: String,
        
        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 私钥文件路径
        #[arg(short = 'i', long)]
        identity_file: Option<String>,
    },
    
    /// 创建远程目录
    Mkdir {
        /// 连接名称或 user@host 格式
        target: String,
        
        /// 远程目录路径
        remote_path: String,
        
        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 私钥文件路径
        #[arg(short = 'i', long)]
        identity_file: Option<String>,
    },
    
    /// 删除远程文件
    Remove {
        /// 连接名称或 user@host 格式
        target: String,
        
        /// 远程文件路径
        remote_path: String,
        
        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 私钥文件路径
        #[arg(short = 'i', long)]
        identity_file: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// 添加新的连接配置
    Add {
        /// 连接名称
        name: String,
        
        /// 主机地址
        host: String,
        
        /// 用户名
        username: String,
        
        /// SSH 端口
        #[arg(short, long, default_value = "22")]
        port: u16,
        
        /// 使用公钥认证
        #[arg(long)]
        use_key: bool,
        
        /// 私钥文件路径
        #[arg(short = 'i', long)]
        identity_file: Option<String>,
        
        /// 公钥文件路径
        #[arg(long)]
        public_key: Option<String>,
    },
    
    /// 列出所有保存的连接
    List,
    
    /// 删除连接配置
    Remove {
        /// 连接名称
        name: String,
    },
    
    /// 设置默认连接
    SetDefault {
        /// 连接名称
        name: String,
    },
    
    /// 显示连接详情
    Show {
        /// 连接名称
        name: String,
    },
    
    /// 显示已保存的密码（需要主密码）
    ShowPassword {
        /// 连接名称（可选，不提供则显示所有）
        name: Option<String>,
    },
}

