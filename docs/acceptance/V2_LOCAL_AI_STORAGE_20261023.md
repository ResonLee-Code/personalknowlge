# V2 本地模型、updater 与同步存储准备记录

## 现场

- 日期：2026-10-23
- 平台：macOS 27.0.1 / arm64
- 设备：Apple M5 Pro，统一内存 24GB，Metal 可用显存约 17.8GB，15 CPU 核
- 可用磁盘：约 639GB
- 候选应用代码：`49de15f`

本记录验证的是本机可执行准备与真实本机互操作，不是 Windows / Linux 物理机结论。

## updater 密钥

- 私钥：`~/.config/pkos-secrets/pkos-updater.key`，权限 `0600`
- 公钥：`~/.config/pkos-secrets/pkos-updater.key.pub`，权限 `0600`
- 私钥密码：macOS Keychain `com.personalknowledge.os.updater`
- 公钥 SHA-256：`67615d8a425f4144b4505bfaa369220525b17a973fe4114b26a94d0887572d32`

验证：

- 使用正式私钥签署临时 `2.0.0` 载荷；
- `updater_signature_flow` 验证有效载荷通过、篡改载荷拒绝；
- 私钥和密码未进入仓库或应用工作区。

安全说明：第一次生成时 `pnpm` 在输出中显示了临时密码。该密钥尚未上传 GitHub，
随后已删除并用新的随机密码重新生成；当前记录和验证均针对轮换后的密钥。

## 本地模型

### 选择

使用 `qwen2.5:7b` 作为 24GB Apple Silicon 的默认本地模型：

- Q4_K_M 权重约 4.7GB；
- 中文生成和摘要质量优于 3B；
- Metal 上 100% GPU，实际进程 RSS 约 4.84GB；
- 仍给应用、WebKit 和系统留下充足内存。14B 可以运行，但内存余量明显变小，暂不作为默认。

### 来源与完整性

- Ollama：0.40.8
- 模型来源：ModelScope 官方仓库 `Qwen/Qwen2.5-7B-Instruct-GGUF`
- 量化：`Q4_K_M`
- 分片校验：
  - 第 1 片：`dfce12e3862a5283ccfb88221b48480e58745165de856439950d0f22590580db`
  - 第 2 片：`539cf93f78e887edea1c04e2d7d8cdaca9d01dae9c9025bcb8accbe29df3d72a`
- 合并后 GGUF SHA-256：`9cee521fb0e89ce49931bc467efdc5bf85946da0a603ef25c473fd4de62f454a`
- 合并文件包含 339 个张量，不再包含 split 标记。

Ollama 导入器不能直接接受 split GGUF，因此使用官方 Python `gguf` 读写库无损合并分片，
并按官方 manifest 的 Qwen chat template / system prompt 导入为 `qwen2.5:7b`。

### 性能

Ollama 直接 API：

- 冷请求墙钟：5.55s，其中模型加载 3.35s；
- 热请求墙钟：1.54s；
- 热生成 72 token，eval 约 48 token/s；
- 100% Metal GPU，4.84GB RSS。

应用内真实 Ask：

- 首次：`1769ms`，provider `local.ollama:qwen2.5:7b`；
- 追问：`905ms`，provider 相同；
- 回答包含 `[1]` 引用；单一来源时应用仍正确显示“证据不足”。

服务由用户级 LaunchAgent `com.ollama.server` 管理，仅监听 `127.0.0.1:11434`，
启用 Flash Attention 与 `q8_0` KV cache。

## 付费在线模型

DeepSeek 作为首选在线档，计划配置：

- Base URL：`https://api.deepseek.com/v1`
- 模型：`deepseek-chat`（实际模型 ID以 DeepSeek 控制台为准）

当前尚未获得用户 API Key，因此没有执行付费请求，也没有用本地模型冒充在线结果。
Key 应直接填入应用“设置 → AI 能力 → 在线模型”，不要发到聊天或提交仓库。

## WebDAV / S3

使用 rclone 1.75.2 启动两个真实本机服务：

- WebDAV：`http://127.0.0.1:8081`
- S3：`http://127.0.0.1:8082`
- S3 bucket：`pkos-sync`
- S3 region：`us-east-1`

凭据存放在：

- `~/.config/pkos-secrets/storage.env`，权限 `0600`
- Keychain：`com.personalknowledge.os.webdav` / `com.personalknowledge.os.s3`
- 同步口令：Keychain `com.personalknowledge.os.sync`

服务由 `com.personalknowledge.os.webdav` 与 `com.personalknowledge.os.s3` 两个用户级
LaunchAgent 管理，只监听回环地址。

协议验证：

- WebDAV：PROPFIND 207、PUT 201、GET 200、DELETE 204，正文一致；
- S3：PUT / GET / DELETE 成功；
- 应用 WebDAV：设备 A 上传，设备 B dry-run 显示下载 1，执行后解密恢复正文和数据库记录；
- 应用 S3：设备 A 上传，设备 B dry-run 显示下载 1，执行后解密恢复正文和数据库记录；
- 两端远端目录均只有 `profile.json`、加密 `manifest.enc` 和随机对象名；
- 对远端目录扫描 `REMOTE-GATE`、`远程门禁`、`RemoteGate`，均未发现明文。

本地安装与操作说明：

- `~/.config/pkos-storage/README.md`

## 尚未完成

- 把 updater 私钥复制到至少一份离线介质，并写入 GitHub Secrets；
- 用户注册 DeepSeek 并填写 API Key 后，执行真实在线模型问答、审阅和抓包；
- 将 WebDAV / S3 从本机回环测试迁移到 HTTPS 可达的真实服务；
- 两台物理设备、断网重试、冲突、历史恢复和 Windows ACL；
- Apple Developer ID / 公证与 Windows Authenticode。
