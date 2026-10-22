# V2 Writing 未授权零外发实测

## 条件

- 真实 DeepSeek Key 已配置；
- 在线写作开关已打开；
- 在线模型未完成首次授权；
- 调用真实 `WritingService::draft`，模式为“大纲”；
- 请求显式传入 `confirmOutbound=true`，证明仅缺少首次授权仍会阻止外发。

## 系统级抓包

测试进程先进入 `before-draft` 状态并打印 PID，`nettop` 完成附着后才开始生成草稿。

```bash
nettop -P -p <test-pid> -L 15 -s 1 -m tcp -J bytes_in,bytes_out
```

15 个连续 1 秒采样只有 CSV 表头，没有测试进程的 TCP 连接行或字节记录。

## 应用结果

- 草稿 tier：`template`；
- 草稿段落仍非空；
- 审计包含 `REMOTE_MODEL_UNCONFIRMED`；
- 审计不包含 API Key。

## 复现

```bash
cred_file="$HOME/Library/Application Support/PersonalKnowledgeOS/ai-credentials.json"
PKOS_LIVE_CAPTURE_READY_MS=10000 \
PKOS_LIVE_HOLD_MS=10000 \
PKOS_LIVE_REMOTE_KEY="$(jq -er '.remoteModelApiKey' "$cred_file")" \
  src-tauri/target/debug/deps/writing_flow-<hash> \
  live_unauthorized_deepseek_writing_keeps_zero_outbound --ignored --nocapture
```

看到 `stage=before-draft` 与 PID 后，在另一终端运行上面的 `nettop` 命令。
