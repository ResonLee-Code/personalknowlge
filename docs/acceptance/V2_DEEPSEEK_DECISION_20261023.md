# V2 DeepSeek 在线模型与决策档实测

## 配置

- 日期：2026-10-23
- 平台：macOS 27.0.1 / arm64
- 在线端点：`https://api.deepseek.com`
- 账户可用模型：`deepseek-flash`、`deepseek-v4-pro`
- 测试模型：`deepseek-flash`
- API Key：已保存到应用数据目录，权限 `0600`；未写入工作区、日志或文档

用户最初填写 `Deepseek-flash`，DeepSeek 返回 HTTP 400。通过带授权的 `/models`
查询确认官方 ID 为全小写 `deepseek-flash`，现已修正。

## 首 token 与总耗时

使用真实 DeepSeek 流式接口、`max_tokens=512`，连续请求三次：

| 轮次 | 首推理 token | 首个正文 token | 总耗时 | reasoning tokens |
| --- | ---: | ---: | ---: | ---: |
| 1 | 1152.9 ms | 1201.6 ms | 1748.1 ms | 66 |
| 2 | 839.6 ms | 1161.0 ms | 1780.5 ms | 55 |
| 3 | 797.9 ms | 1885.1 ms | 2303.5 ms | 139 |

- 首推理 token 中位数：**839.6 ms**
- 首个正文 token 中位数：**1201.6 ms**
- 总耗时中位数：**1780.5 ms**
- 应用 Ask 使用非流式请求，12 次成功远程请求的总耗时范围为 **1175–1824 ms**，中位数
  **1412.5 ms**，均值 **1436.8 ms**。

## 引用准确率

建立 10 个独立笔记，每题只有一个唯一标记和一个明确来源：

- 最终交付答案：**10/10** 包含正确唯一标记、带数字引用，并显示对应笔记的来源按钮；
- 被接受在线答案的引用支持：**10/10**；
- 首轮在线引用合规：**9/10（90%）**。其中一次 DeepSeek 回答没有引用任何材料，应用
  将其整份丢弃并降级模板；重新提问后在线答案通过校验。

结论：引用守卫能阻止“有答案但无引用”的结果，但本次小样本的首轮在线引用合规率未达到
95% 人工目标，不能把最终 10/10 说成模型首轮已达到 95%。

## 失败降级

1. 错误模型大小写：DeepSeek HTTP 400，应用降级模板，仍保留正确 `[1]` 引用。
2. 本机不可达端点：`Connection refused` 后降级模板，正文和引用正确。
3. 在线回答无引用：整份在线答案被丢弃，提示引用校验失败并降级。
4. 未勾选逐次外发确认：审计记录 `OUTBOUND_UNCONFIRMED`，系统级 `nettop` 无连接，
   模板回答正常返回。

审计日志只包含 capability、时间、耗时、outcome 与短码，没有测试问题、唯一标记或模型完整
提示词。

## 决策模型分析

### 地址

`https://api.perplexity.ai` 是 API 基地址，不是可浏览网页。浏览器直接打开或请求根路径
出现失败/超时是正常现象。项目实际请求：

```text
POST https://api.perplexity.ai/v1/decisions
```

未携带合法 Key 时该地址返回 **401 invalid_api_key**，证明 URL 和路径可达。

### 已修复的问题

- 后端 `ai_policy_set` 原先只把在线模型 Key 写入凭据文件，完全忽略
  `decisionApiKey`，导致决策 Key 永远显示未配置。
- 前端在保存结果返回前就清空 Key 输入框，保存失败也会丢失用户唯一副本。

现已修复：

- 在线模型与决策模型 Key 分别持久化，互不清除；
- 只有后端确认保存成功后前端才清空 Key；
- 新增 Rust 凭据持久化和前端失败保留输入的回归测试。

### 如何让决策档生效

1. 使用修复后的 `.app`，重新进入“设置 → AI → 决策模型”；
2. 档位选择 `System One`；
3. 填写具有 Perplexity Decisions 权限的 Key；
4. 点击“保存 Key”；界面应显示 “已配置”；
5. 点击“授权决策模型”，确认外发；
6. 点击“运行契约探针”，只有返回准确率、校准等报告才算生效。

DeepSeek Key 不能用于 Perplexity 的 `/v1/decisions` 契约。若没有 Perplexity Decisions
权限：

- 把决策档切回“规则档”；
- 或打开本地模型区的“用本地模型打分”，让 `qwen2.5:7b` 参与 Research 打分链；
- 或打开在线模型区的“用在线模型打分”，让 DeepSeek 参与 Research 打分链。

后两者使用普通文本模型 JSON 打分，不会让专门的“System One 契约探针”通过。

Ask 50 题完整在线评测与人工复核边界另见
[V2 Ask 真实 DeepSeek 50 题评测](./V2_ASK_LIVE_DEEPSEEK_20261023.md)。

### 最终应用复验

- 重新构建 macOS release `.app`，`codesign --verify --deep --strict` 通过；
- 最终应用成功打开真实工作区 `/Users/reson/MyProjects/knowledge`；
- 使用明显无效的本地假 Key 验证决策 Key 可独立保存并清除，过程中没有发出模型请求；
- 清除后凭据文件权限仍为 `0600`，在线 DeepSeek Key 保留，决策 Key 为空；
- 真实 Perplexity Decisions Key、决策授权和契约探针仍待人工提供凭据后执行。
