# V2 AI 总开关开启与本地性能冒烟

## 范围

- 日期：2026-10-22
- 候选代码：`49de15fbb17cbc468157ef2c477e901c4cffef9a`
- 平台：macOS 27.0.1 / arm64
- 应用：`Personal Knowledge OS.app`，ad-hoc 签名，版本 1.3.7
- 隔离工作区：`/private/tmp/pkos-v2-49de15f-native.GrpHvS/workspace`

本报告验证打开 AI 总开关后的本地 / 模板路径、实际档位、审计耗时和零外发边界。
本机没有 Ollama，也没有在线模型凭据，因此**不能**用本报告评价真实模型质量。

## 操作与证据

1. 通过真实 UI 进入「设置 → AI 能力」，点击「AI 能力总开关」。
2. 重新读取工作区设置，确认 `ai.enabled = true`，联网档位仍为 `localOnly`。
3. 点击「开始自检」，界面结论为：
   - 当前档位：**模板实现（`template.draft`）**
   - 在线模型：未开启
   - 本地模型：未开启
   - 模板实现：不联网、不调用模型，结论明确标注「未接模型」
4. 在 Ask 中选择「全库」，提问「验收-备份恢复笔记里写了什么？」：
   - 返回 **8 条可引用材料**
   - provider：`template.ask`
   - 界面记录：`2ms`
   - 页面操作墙钟：约 1.6 秒，包含 AX 查询与渲染，不作为后端延迟
5. 在 Research 中以「备份恢复」为题走本地全链路：
   - 收集：1 条本地来源
   - 验证：报告明确因独立来源不足而不给结论
   - 汇总：`template.draft`
   - 审计耗时：立项 `5ms`、收集 `4ms`、验证 `7ms`、汇总 `5ms`

AI 审计文件：

```text
/private/tmp/pkos-v2-49de15f-native.GrpHvS/workspace/.knowledge/logs/ai-audit.jsonl
```

关键记录：

```text
ask_answer        outbound=false durationMs=2 note=template.ask
research_start    outbound=false durationMs=5 note=null
research_collect  outbound=false durationMs=4 note=local_only
research_verify   outbound=false durationMs=7 note=null
research_compose  outbound=false durationMs=5 note=template.draft
```

## 结论

- AI 总开关开启、策略持久化、模型自检、Ask 和 Research 五步链路均正常。
- 没有可用模型时如实降级到模板档，不把模板输出冒充模型输出。
- 该轮 Ask / Research 的所有审计均为 `outbound=false`。
- 真实在线模型、本地 Ollama 模型、模型质量、真实网络抓包仍属于统一人工验收项。
