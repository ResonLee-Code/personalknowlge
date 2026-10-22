# V2 Organize 摘要事实预检

## 范围

- 日期：2026-10-23
- 模型：`deepseek-flash`
- 入口：真实 `OrganizeService::generate_with_confirmation`
- 样本：10 个技术主题，每个主题两篇，共 20 篇合成笔记
- 每篇正文包含 3 个可核对事实
- 生成阶段只写 pending 建议，不修改 Markdown

## 结果

| 指标 | 结果 |
| --- | ---: |
| 用例 | 20 |
| 在线模型成功 | 20 |
| 降级到规则档 | 0 |
| 关键事实覆盖 | 60 / 60 |
| 关键事实覆盖率 | **100.00%** |
| 跨主题虚构标记 | 0 |
| 总耗时 | 41.0 s |

摘要均保持 2–4 个要点，能从正文复述技术事实。部分输出会补一句“材料信息有限”或
“仅提供一条来源”，属于保守说明，不是新增事实。

## 边界

该测试只能确认：

- 预期关键事实没有被明显漏掉；
- 没有出现预定义的跨主题虚构标记；
- 真实模型没有静默降级。

它不能证明每句话的语义都由原文支撑，也不能替代真实知识库的人工事实一致性抽检。

## 复现

```bash
cred_file="$HOME/Library/Application Support/PersonalKnowledgeOS/ai-credentials.json"
PKOS_LIVE_SUMMARY_CASES=20 \
PKOS_LIVE_REMOTE_KEY="$(jq -er '.remoteModelApiKey' "$cred_file")" \
  cargo test --offline --manifest-path src-tauri/Cargo.toml \
  --test organize_flow live_deepseek_summary_fact_precheck \
  -- --ignored --nocapture
```
