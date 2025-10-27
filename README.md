# Rust SSH/SFTP Client

一个功能强大的跨平台 SSH/SFTP 客户端，使用 Rust 编写，支持交互式终端、密码加密保存、图形界面等功能。

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org/)

## ✨ 特性

- 🔐 **多种认证方式**: 密码认证（支持加密保存）、公钥认证、主密码保护（AES-256-GCM）
- 💻 **交互式终端**: 完整 PTY 支持、异步 I/O、CPR 序列过滤
- 🖥️ **图形界面**: 基于 egui 的现代 GUI、连接管理、一键连接
- 📋 **交互式菜单**: 终端选择菜单、保存的连接列表
- 📁 **SFTP 文件传输**: 上传/下载文件、列出目录、进度条显示
- ⚙️ **配置管理**: 保存常用连接、加密密码存储、密码查看功能

## 🚀 快速开始

### 编译安装

```bash
# 克隆仓库
git clone https://github.com/yourusername/rust-ssh-sftp.git
cd rust-ssh-sftp

# 编译（Release 版本）
cargo build --release

# 二进制文件位于
./target/release/rust-ssh-sftp
```

### 使用方式

#### 1. 图形界面模式
```bash
rust-ssh-sftp gui
```

#### 2. 交互式菜单模式
```bash
# 显示已保存的连接列表，选择连接
rust-ssh-sftp connect -I
```

#### 3. 直接连接
```bash
# 密码认证
rust-ssh-sftp connect user@example.com -I

# 使用私钥认证
rust-ssh-sftp connect user@example.com -I -i ~/.ssh/id_rsa

# 保存密码（加密存储）
rust-ssh-sftp connect user@example.com -I --save-password --save-as "我的服务器"
```

## 📚 主要功能

### SSH 连接

```bash
# 连接到服务器
rust-ssh-sftp connect user@host -I

# 指定端口
rust-ssh-sftp connect user@host -p 2222 -I

# 使用保存的连接
rust-ssh-sftp connect myserver -I

# 执行远程命令
rust-ssh-sftp exec myserver "ls -la"
```

### SFTP 文件传输

```bash
# 上传文件
rust-ssh-sftp sftp upload myserver /local/file.txt /remote/file.txt

# 下载文件
rust-ssh-sftp sftp download myserver /remote/file.txt /local/file.txt

# 列出远程目录
rust-ssh-sftp sftp list myserver /remote/path

# 创建远程目录
rust-ssh-sftp sftp mkdir myserver /remote/newdir

# 删除远程文件
rust-ssh-sftp sftp remove myserver /remote/file.txt
```

### 配置管理

```bash
# 列出所有保存的连接
rust-ssh-sftp config list

# 添加新连接
rust-ssh-sftp config add myserver example.com user -p 22

# 显示连接详情
rust-ssh-sftp config show myserver

# 查看已保存的密码（需要主密码）
rust-ssh-sftp config show-password myserver

# 设置默认连接
rust-ssh-sftp config set-default myserver

# 删除连接
rust-ssh-sftp config remove myserver
```

## 🔒 安全特性

### 密码加密

所有保存的密码都使用**军事级加密**：

- **AES-256-GCM**: 对称加密算法，256位密钥
- **Argon2**: 密钥派生函数，防止暴力破解
- **主密码保护**: 需要主密码才能解密已保存的密码
- **随机 Nonce**: 每次加密使用不同的随机数

### 配置文件位置

- **Linux**: `~/.config/rust-ssh-sftp/config.toml`
- **Windows**: `C:\Users\<用户名>\AppData\Roaming\rust-ssh-sftp\config.toml`
- **macOS**: `~/Library/Application Support/rust-ssh-sftp/config.toml`

密码以加密形式存储，无法直接从配置文件读取。

## 📖 详细文档

- [使用指南](docs/USAGE_GUIDE.md) - 详细的使用说明
- [密码查看功能](docs/PASSWORD_VIEW_GUIDE.md) - 如何查看已保存的密码
- [测试报告](docs/TESTING_REPORT.md) - 功能测试记录
- [项目完成总结](docs/PROJECT_COMPLETION_SUMMARY.md) - 开发总结

## 🛠️ 技术栈

- **russh** - SSH 协议实现
- **russh-sftp** - SFTP 协议实现
- **tokio** - 异步运行时
- **crossterm** - 跨平台终端操作
- **eframe/egui** - GUI 框架
- **aes-gcm** - AES-256-GCM 加密
- **argon2** - 密钥派生函数
- **clap** - 命令行参数解析

## 🧪 构建要求

- Rust 1.70 或更高版本
- Cargo

### 依赖项

在大多数系统上，cargo 会自动处理所有依赖项。对于某些系统，可能需要额外的开发包：

**Ubuntu/Debian:**
```bash
sudo apt-get install build-essential pkg-config libssl-dev
```

**Fedora/RHEL:**
```bash
sudo dnf install gcc pkg-config openssl-devel
```

**macOS:**
```bash
# 使用 Homebrew
brew install pkg-config openssl
```

## 📝 使用示例

### 基本工作流程

```bash
# 1. 首次连接并保存密码
rust-ssh-sftp connect user@server.com -I --save-password --save-as "生产服务器"
# 系统会提示设置主密码

# 2. 以后直接使用保存的连接
rust-ssh-sftp connect "生产服务器" -I
# 输入主密码后自动连接

# 3. 查看所有连接
rust-ssh-sftp config list

# 4. 忘记密码时查看
rust-ssh-sftp config show-password "生产服务器"
# 输入主密码后显示解密的密码
```

### SFTP 批量操作

```bash
# 上传整个目录（需要先创建远程目录）
rust-ssh-sftp sftp mkdir myserver /remote/backup
rust-ssh-sftp sftp upload myserver ./local_dir/file1.txt /remote/backup/
rust-ssh-sftp sftp upload myserver ./local_dir/file2.txt /remote/backup/
```

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

## ⚠️ 免责声明

- 请妥善保管主密码，忘记后无法恢复已保存的密码
- 建议定期备份配置文件
- 在公共计算机上使用时请注意安全

## 🙏 致谢

- [russh](https://github.com/warp-tech/russh) - SSH 协议实现
- [egui](https://github.com/emilk/egui) - 即时模式 GUI 库
- 所有开源贡献者

## 📮 联系方式

如有问题或建议，请通过以下方式联系：

- 提交 [GitHub Issue](https://github.com/yourusername/rust-ssh-sftp/issues)
- 发送邮件至: your.email@example.com

---

**⭐ 如果这个项目对你有帮助，请给一个 Star！**
