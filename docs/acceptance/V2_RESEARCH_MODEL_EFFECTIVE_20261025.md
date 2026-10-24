# Research 在线模型生效核验与旧文案修复

## 结论

当前 DeepSeek 配置本身有效。用户截图对应的研究会话已经由在线模型完成报告汇总，但来源评分
仍由规则档执行。两组信号分别对应两个独立开关，不能只看“在线模型已配置”：

| 项目 | 实际状态 | 结果 |
| --- | --- | --- |
| AI 总开关 | 开启 | 研究流程可运行 |
| 联网档位 | `allowNetwork` | 联网模式已打开 |
| 检索源地址 | 未配置 | 收集阶段只读本地材料，外部来源为 0 |
| 在线模型 Key | 已配置 | DeepSeek 请求可鉴权 |
| 在线模型授权 | 已授权 | 允许材料片段外发 |
| 用在线模型写报告 | 开启 | 报告 `composer = remote.openai-compatible:deepseek-flash` |
| 用在线模型打分 | 关闭 | `decision_provider = rule`，来源可信度仍按规则计算 |

截图所示会话 `research_01M5Q9YVPC1VRMEC07Z57SX8JW` 的证据：

- 状态：`composed`
- 本地来源：15
- 外部来源：0
- 打分：`rule`
- 汇总模型：`remote.openai-compatible:deepseek-flash`
- 汇总耗时：9549 ms

因此“外部模型完全没调用”并不成立；实际是**在线汇总生效、在线评分未启用、外部检索未配置**。

## 真实 DeepSeek 复验

使用当前已配置 Key，经本机代理显式启用 `remoteModelScore` 与 `remoteModelCompose`：

```text
PKOS_LIVE_RESEARCH_SCORE provider=remote.openai-compatible sources=2 trace_attempts=1 elapsed_ms=1363
PKOS_LIVE_RESEARCH_SOURCE title=Rust 异步调度方案对比 credibility=0.700
PKOS_LIVE_RESEARCH_SOURCE title=Rust 异步调度实现要点 credibility=0.650
PKOS_LIVE_RESEARCH_REPORT composer=remote.openai-compatible:deepseek-flash sources=2 warnings=2 elapsed_ms=8001
```

该结果证明在线打分和在线汇总均能真实生效，Key 未写入日志。

## 已修复

1. 研究计划不再硬编码“本次只用本地材料（外部检索属于阶段 D3）”和“等模型接入”；
   现在分别显示联网源、来源评分、报告汇总的真实状态。
2. 研究页头读取实际联网策略；若在线模型已配置但打分关闭，明确提示来源可信度与冲突判定
   仍由规则档处理。
3. 报告中的打分来源区分“在线模型 / 本地模型 / 规则档”。
4. 在线阅读或打分成功后，审计不再错误记录 `outbound=false`；报告汇总、来源打分和外部检索
   分别按实际请求记录外发。

## 用户侧生效步骤

1. 打开「设置 → AI → 在线模型」，确认 DeepSeek 地址、模型名、Key 和授权状态。
2. 勾选「用在线模型打分」；若要生成推测段，同时勾选「用在线模型写报告」。
3. 若需要外部网页来源，另行打开「允许联网检索」并配置返回 JSON 的检索源地址；
   在线模型配置不会自动配置搜索服务。
4. 新建研究并重新执行收集、验证、汇总。既有报告不会因为事后改开关而自动重算。

## 边界

本记录证明模型调用链真实有效；搜索结果可信度、报告事实质量和引用支持仍需人工评估。
