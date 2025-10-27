# 使用指南

## 快速开始

### 1. 编译程序

```bash
cargo build --release
```

编译后的可执行文件位于 `target/release/rust-ssh-sftp.exe`（Windows）或 `target/release/rust-ssh-sftp`（Linux）。

### 2. 三种使用方式

#### 方式一：图形界面（推荐新手）

```bash
.\target\release\rust-ssh-sftp.exe gui
```

在 GUI 中可以：
- 查看所有保存的连接
- 添加新连接
- 删除连接
- 保存加密密码
- 一键连接

#### 方式二：交互式菜单（推荐）

```bash
.\target\release\rust-ssh-sftp.exe connect -I
```

会显示：
```
=== 已保存的连接 ===

  [1] root@158.69.121.75 root@158.69.121.75:17060 🔒

  [0] 手动输入连接信息
  [q] 退出

请选择连接 [1-1, 0=手动, q=退出]:
```

- 输入数字选择连接
- 输入 0 手动输入连接信息
- 输入 q 退出

#### 方式三：命令行直接连接

```bash
# 基本连接
.\target\release\rust-ssh-sftp.exe connect user@host -I

# 指定端口
.\target\release\rust-ssh-sftp.exe connect user@host -p 2222 -I

# 保存密码
.\target\release\rust-ssh-sftp.exe connect user@host -I --save-password --save-as "我的服务器"
```

## 密码管理

### 首次使用

第一次保存密码时，程序会要求创建主密码：

```
请创建主密码（用于加密保存的密码）: ********
请再次输入主密码确认: ********
```

**重要**：请牢记主密码！忘记主密码将无法解密已保存的密码。

### 保存密码

```bash
# 连接时保存密码
.\target\release\rust-ssh-sftp.exe connect user@host -I --save-password

# 保存为命名连接
.\target\release\rust-ssh-sftp.exe connect user@host -I --save-password --save-as "生产服务器"
```

### 使用已保存的密码

```bash
# 使用连接名
.\target\release\rust-ssh-sftp.exe connect "生产服务器" -I

# 或使用交互式菜单
.\target\release\rust-ssh-sftp.exe connect -I
```

程序会提示输入主密码，然后自动解密并使用保存的 SSH 密码。

## 配置管理

### 查看所有连接

```bash
.\target\release\rust-ssh-sftp.exe config list
```

输出示例：
```
保存的连接:

* [root@158.69.121.75] root@158.69.121.75:17060 (password) 🔑

提示:
  * 表示默认连接
  🔑 表示已保存密码
```

### 查看连接详情

```bash
.\target\release\rust-ssh-sftp.exe config show "连接名"
```

### 删除连接

```bash
.\target\release\rust-ssh-sftp.exe config remove "连接名"
```

## SFTP 文件传输

### 上传文件

```bash
.\target\release\rust-ssh-sftp.exe sftp upload user@host local.txt /remote/path/file.txt
```

### 下载文件

```bash
.\target\release\rust-ssh-sftp.exe sftp download user@host /remote/file.txt local.txt
```

### 列出远程目录

```bash
.\target\release\rust-ssh-sftp.exe sftp list user@host /remote/path
```

## 执行远程命令

```bash
# 执行单个命令
.\target\release\rust-ssh-sftp.exe exec user@host "ls -la"

# 执行多个命令
.\target\release\rust-ssh-sftp.exe exec user@host "cd /var/log && tail -n 20 syslog"
```

## 交互式终端使用

连接成功后，您会看到：

```
=== 交互式 SSH Shell ===
连接到: root@158.69.121.75
按 Ctrl+D 或输入 'exit' 退出
========================

[欢迎消息]

root@host:~#
```

在交互式终端中：
- 所有命令都会实时执行
- 支持彩色输出
- 支持特殊按键（方向键、Tab 补全等）
- 按 `Ctrl+D` 或输入 `exit` 退出

## 常见问题

### Q: 忘记主密码怎么办？

A: 主密码无法恢复。您需要：
1. 删除配置文件（Windows: `C:\Users\<用户名>\AppData\Roaming\rust-ssh-sftp\config.toml`）
2. 重新创建主密码
3. 重新保存所有连接的密码

### Q: 如何更改主密码？

A: 目前不支持更改主密码。您需要：
1. 删除配置文件
2. 重新创建主密码
3. 重新保存所有连接

### Q: 密码保存在哪里？

A: 配置文件位置：
- **Windows**: `C:\Users\<用户名>\AppData\Roaming\rust-ssh-sftp\config.toml`
- **Linux**: `~/.config/rust-ssh-sftp/config.toml`

密码使用 AES-256-GCM 加密，非常安全。

### Q: 如何启用调试日志？

A: 设置环境变量：

**Windows PowerShell**:
```powershell
$env:RUST_LOG="debug"
.\target\release\rust-ssh-sftp.exe connect user@host -I
```

**Linux/Mac**:
```bash
RUST_LOG=debug ./target/release/rust-ssh-sftp connect user@host -I
```

### Q: 连接失败怎么办？

A: 检查：
1. 主机地址和端口是否正确
2. 用户名和密码是否正确
3. 网络连接是否正常
4. 防火墙是否阻止连接
5. 查看调试日志（启用 `RUST_LOG=debug`）

## 高级用法

### 使用公钥认证

```bash
# 指定私钥文件
.\target\release\rust-ssh-sftp.exe connect user@host -i ~/.ssh/id_rsa -I

# 如果私钥有密码保护，程序会提示输入
```

### 批量操作

```bash
# 上传多个文件（使用脚本）
for file in *.txt; do
    .\target\release\rust-ssh-sftp.exe sftp upload user@host "$file" "/remote/$file"
done
```

## 安全建议

1. **使用强主密码**：至少 12 个字符，包含大小写字母、数字和特殊字符
2. **定期更换密码**：建议每 3-6 个月更换一次 SSH 密码
3. **使用公钥认证**：比密码认证更安全
4. **保护配置文件**：确保配置文件权限正确（仅当前用户可读）
5. **不要共享主密码**：主密码应该只有您自己知道

## 性能优化

1. **使用已保存的连接**：避免每次都输入连接信息
2. **使用公钥认证**：比密码认证更快
3. **关闭调试日志**：生产环境不要启用 `RUST_LOG=debug`

## 更新日志

### v1.0.0 (2025-10-25)

- ✅ 基于 russh 的异步 SSH 连接
- ✅ 交互式终端支持
- ✅ CPR 序列过滤
- ✅ 密码加密保存（AES-256-GCM）
- ✅ 交互式连接选择菜单
- ✅ 图形界面（基于 egui）
- ✅ SFTP 文件传输
- ✅ 配置管理

