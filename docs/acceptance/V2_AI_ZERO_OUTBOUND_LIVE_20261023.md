# V2 在线模型未授权零外发实测

## 条件

- 日期：2026-10-23
- 真实 DeepSeek Key 已写入临时应用数据目录；
- 端点：`https://api.deepseek.com`；
- 模型：`deepseek-flash`；
- 在线写作和在线打分开关都已打开；
- **没有完成在线模型首次授权**。

Key 只从应用数据目录注入测试进程，没有进入命令输出、测试报告或工作区。

## 系统级抓包

测试进程先进入 `before-research` 状态并打印 PID，给 `nettop` 留出附着时间，然后才执行
Research 的立项、收集、验证和汇总。

观察命令：

```bash
nettop -P -p <test-pid> -L 15 -s 1 -m tcp -J bytes_in,bytes_out
```

结果：

- 15 个连续 1 秒采样只有 CSV 表头；
- 没有测试进程的 TCP 连接行；
- `bytes_in` / `bytes_out` 均没有记录。

## 应用结果

- 来源打分 provider：`rule`；
- 报告 composer：`template.draft`；
- 审计包含 `REMOTE_MODEL_UNCONFIRMED`；
- 审计与输出不包含 API Key。

结论：真实 Key 已配置、在线开关已打开时，仅缺少授权仍会在结构上阻止在线模型构造，
Research 正常降级且没有外发。

## 复现

```bash
cred_file="$HOME/Library/Application Support/PersonalKnowledgeOS/ai-credentials.json"
PKOS_LIVE_CAPTURE_READY_MS=10000 \
PKOS_LIVE_HOLD_MS=10000 \
PKOS_LIVE_REMOTE_KEY="$(jq -er '.remoteModelApiKey' "$cred_file")" \
  src-tauri/target/debug/deps/remote_model_flow-<hash> \
  live_unauthorized_deepseek_keeps_zero_outbound --ignored --nocapture
```

看到 `stage=before-research` 与 PID 后，在另一终端运行上面的 `nettop` 命令。
