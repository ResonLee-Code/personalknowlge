# V2 DeepSeek Research 打分与汇总实测

## 配置口径

该测试不使用 System One / Perplexity Decisions：

- 决策模型档：`rule`
- 在线模型地址：`https://api.deepseek.com`
- 在线模型名：`deepseek-flash`
- `remoteModelScore = true`
- `remoteModelCompose = true`
- 已完成在线模型首次授权

在线模型 Key 只从应用数据目录注入临时测试进程，不进入输出或工作区。

## 结果

| 阶段 | provider | 结果 | 耗时 |
| --- | --- | --- | ---: |
| 研究收集 / 来源打分 | `remote.openai-compatible` | 2 个来源，trace 1 次尝试 | 1.802 s |
| 来源可信度 | DeepSeek | 0.525 / 0.625 | — |
| 研究验证 / 汇总 | `remote.openai-compatible:deepseek-flash` | 报告含 2 个来源、2 条警告 | 7.779 s |

断言确认：

- `decisionProvider` 实际为 `remote.openai-compatible`，不是规则档兜底；
- `decisionTrace` 存在且记录本次在线打分尝试；
- 每个来源都有正可信度；
- 报告 `composer` 为 `remote.openai-compatible:deepseek-flash`；
- 报告带“模型生成、未经核对”警告。

## 如何让模型生效

只有 DeepSeek Key 时：

1. 决策模型档保持“规则档”；
2. 在“在线模型”区域填写 `https://api.deepseek.com` 和 `deepseek-flash`；
3. 确认 Key 显示“已配置”；
4. 打开“用在线模型打分”；
5. 如需要模型写报告，同时打开“用在线模型写报告”；
6. 先完成一次在线模型授权，再运行 Research。

如果仍选择 System One：

- DeepSeek Key 不会生效；
- 必须提供具有 Perplexity Decisions 权限的独立 Key；
- 保存 Key、授权决策模型并运行契约探针，探针成功后该档才真正生效。

## 复现

```bash
cred_file="$HOME/Library/Application Support/PersonalKnowledgeOS/ai-credentials.json"
PKOS_LIVE_REMOTE_KEY="$(jq -er '.remoteModelApiKey' "$cred_file")" \
  cargo test --offline --manifest-path src-tauri/Cargo.toml \
  --test remote_model_flow live_deepseek_scores_and_writes_research \
  -- --ignored --nocapture
```
