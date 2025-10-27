# 查看已保存密码功能说明

## ✨ 功能介绍

新增了 `show-password` 命令，让你可以使用主密码解密并查看已保存的 SSH 密码。

## 🔐 安全说明

- **需要主密码**: 必须输入正确的主密码才能解密
- **加密存储**: 密码使用 AES-256-GCM 加密保存
- **不会明文记录**: 密码只在终端临时显示，不会保存到日志

## 📋 使用方法

### 1. 查看所有已保存密码的连接

```bash
rust-ssh-sftp config show-password
```

输出示例：
```
需要主密码来解密保存的密码
请输入主密码: ********

已保存的密码:

  [我的服务器]
    主机:     user@example.com:22
    认证方式: password
    密码:     MyPassword123

  [测试服务器]
    主机:     admin@test.com:22
    认证方式: publickey
    私钥:     /home/user/.ssh/id_rsa
    私钥密码: KeyPassphrase456

⚠️  请注意保护好这些密码信息！
```

### 2. 查看指定连接的密码

```bash
rust-ssh-sftp config show-password 我的服务器
```

输出示例：
```
需要主密码来解密保存的密码
请输入主密码: ********

已保存的密码:

  [我的服务器]
    主机:     user@example.com:22
    认证方式: password
    密码:     MyPassword123

⚠️  请注意保护好这些密码信息！
```

## 🎯 命令格式

```bash
# 完整命令格式
rust-ssh-sftp config show-password [连接名称]

# 参数说明
[连接名称]  可选参数，指定要查看的连接名称
           如果不提供，则显示所有已保存密码的连接
```

## 📝 注意事项

### ✅ 支持的场景

- ✓ 密码认证：显示 SSH 登录密码
- ✓ 公钥认证：显示私钥的密码（passphrase）
- ✓ 批量查看：一次显示所有连接的密码
- ✓ 单个查看：只显示指定连接的密码

### ⚠️ 可能的错误

1. **"未设置主密码，无法解密"**
   - 原因：配置目录中没有 `.salt` 文件
   - 解决：先保存一个带密码的连接

2. **"解密失败（可能是主密码错误）"**
   - 原因：输入的主密码不正确
   - 解决：重新输入正确的主密码

3. **"连接 'xxx' 不存在"**
   - 原因：指定的连接名称不存在
   - 解决：使用 `rust-ssh-sftp config list` 查看可用连接

4. **"连接 'xxx' 没有保存密码"**
   - 原因：该连接创建时没有选择保存密码
   - 解决：重新连接并使用 `--save-password` 选项

## 🔍 查看所有连接

如果不确定哪些连接保存了密码，可以先列出所有连接：

```bash
rust-ssh-sftp config list
```

输出中带有 🔑 图标的连接表示已保存密码：

```
保存的连接:

* [我的服务器] user@example.com:22 (password) 🔑
  [测试服务器] admin@test.com:22 (publickey) 🔑
  [开发服务器] dev@dev.com:22 (password)

提示:
  * 表示默认连接
  🔑 表示已保存密码
```

## 🛡️ 安全建议

1. **不要在共享屏幕时查看密码**
2. **查看后及时清除终端历史**
3. **在公共场所谨慎使用此功能**
4. **定期更换主密码**
5. **妥善保管主密码，丢失后无法恢复已保存的密码**

## 💡 使用场景

- 忘记某个服务器的密码，需要查看
- 迁移到新设备前导出密码
- 验证保存的密码是否正确
- 审计已保存的连接信息

## 🔄 配合其他命令使用

```bash
# 1. 查看所有连接
rust-ssh-sftp config list

# 2. 查看指定连接的详细信息（不包含密码）
rust-ssh-sftp config show 我的服务器

# 3. 查看该连接的密码
rust-ssh-sftp config show-password 我的服务器

# 4. 直接使用保存的密码连接
rust-ssh-sftp connect 我的服务器 -I
```

## 技术细节

### 加密算法
- **对称加密**: AES-256-GCM
- **密钥派生**: Argon2
- **密钥长度**: 256 位
- **随机 Nonce**: 12 字节

### 存储位置
- **Windows**: `C:\Users\<用户名>\AppData\Roaming\rust-ssh-sftp\config.toml`
- **Linux**: `~/.config/rust-ssh-sftp/config.toml`

### 配置文件示例
```toml
[connections.我的服务器]
name = "我的服务器"
host = "example.com"
port = 22
username = "user"
auth_type = "password"
encrypted_password = "base64_encoded_encrypted_data..."
```

密码字段 `encrypted_password` 是经过加密的，无法直接看出原始密码。

