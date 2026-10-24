# Decision Model 联网候选筛选验收

## 范围

需求 1 独立于尚未实施的“内容发现”功能：研究助手通过 SearXNG 自动搜索网页后，
由已授权的 System One / Perplexity Decisions 模型批量判断候选网页与课题的相关性，
筛除明确低相关的候选，并把结果写入会话、步骤日志和报告。

## 行为

- Decision Model 只调用一次，同一次结果同时用于候选筛选和最终 `credibility`。
- `System One` / `Jev` / 在线模型 / 本地模型档：
  `choice = Drop` 且 `score < 0.25` 的网页从最终来源集合移除。
- 规则档只评分，不据此删除网页，避免离线兜底误伤召回。
- 被筛除项写入 `droppedSources[].reason = decisionLowRelevance`。
- 报告生成 `WarningKind::DecisionFiltered`，列出筛除数量与涉及域名。
- Decision 失败时继续回退本地或规则档，研究流程不中断。
- 审计不包含网页正文与 Key。

## 自动化验证

确定性测试：

```text
test decision_model_filters_clearly_irrelevant_web_candidates_and_reports_it ... ok
```

该测试让 Decision 桩把全部网页判为 `Drop/0.05`，断言最终来源为空、
`decisionLowRelevance` 留痕，并确认报告中出现 `decisionFiltered` 警告。

## 真实联调

本机 SearXNG + 已配置的 Perplexity Decisions：

```text
PKOS_LIVE_SEARXNG_DECISION provider=system_one web_sources=11 decision_filtered=2
```

这证明真实搜索候选进入了 Decision 模型，模型筛选结果被应用到最终来源集合；
Key 与网页正文均未输出。
