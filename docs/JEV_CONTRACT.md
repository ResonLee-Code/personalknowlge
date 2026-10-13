# TypeSafe / Jev 契约速查（D4 / N8 用）

> **2026-10-13 更新（已落地）**：Jev 与 **Perplexity Decisions** 的协议**完全相同**
> （请求 `{model, state, questions}` → 响应 `{model, answers, usage}`；答案类型
> `noul` / `choice` / `score`，字段名一一对应）。所以仓库里只实现了**一个**适配器
> （`application/ai/system_one.rs`），靠 `base_url + model + key` 切换两家；
> 「设置 → AI → 决策模型」里内置两套预设。
>
> **Perplexity Decisions 的确定性要点**（已用真实请求核实）：
> - 端点 `POST https://api.perplexity.ai/v1/decisions`（**不能带尾斜杠**；`x-api-key` 不读 → 401）；
> - 模型 `pplx-decider-v1.1-27b` / `pplx-decider-v1-27b`；**$0.02 / 百万输入 token**，输出免费；
> - 上限：问题 1–128、choice 选项 ≤255、score 层级 ≤10、输入 <262,144 token、body <32 MiB；
> - 限流 **10 请求/秒/组织**，超限 429 + `Retry-After`；成功与多数错误带 `x-request-id`；
> - 图片只接受 **base64 data URL**（http(s) URL → 400）；大图不报 400 而是约一分钟后 504；
> - 相同请求通常返回**完全相同的数字**（我们连跑三次逐位一致）。

> 依据官方文档整理：<https://docs.typesafe.ai/api>、<https://docs.typesafe.ai/models>、
> <https://docs.typesafe.ai/introduction/quickstart>（抓取日期 2026-10-12）。
> **本文件只记录我们接入需要的事实**；权威口径以官方文档为准，实现前请复核。

## 是什么

Jev 是 TypeSafe 的 **System One** 决策模型（不是聊天模型）：**给它 state，问它类型化的问题，
拿回结构化答案**，答案带**校准过的概率与置信度**。这正对上本仓库的决策层
（`DecisionModel` + `DecisionResult{score, confidence, choice, reason}`）。

## 端点与鉴权

```http
POST https://api.typesafe.ai/v1/systemone
Authorization: Bearer <API_KEY>
Content-Type: application/json
```

- Key 从 <https://console.typesafe.ai/keys> 获取；`GET /v1/models` 列出账号可用的模型名。
- 一次调用 = **一个 state + 一组问题**；问题并行评估（官方称批量提问比逐条便宜一个数量级）。

## 请求

```json
{
  "state": "<文本 | 对象 | 数组>",
  "model": "jev-latest",
  "questions": {
    "<自定义 id>": {
      "type": "noul | choice | score",
      "instructions": "<问题；也可用对象/数组携带数据，用反引号引用字段名>",
      "criteria": { }
    }
  }
}
```

- `noul`：`criteria` 可选 `{ "true": "...", "false": "..." }`；
- `choice`：`criteria` 是**对象**，键=选项、值=说明；
- `score`：`criteria` 是**有序数组**，每个元素是一档的说明；
- `instructions` 可以是字符串 / 对象 / 数组（把数据放进对象字段、用反引号引用）。

## 响应

```json
{
  "model": "jev-1.13.0",
  "answers": {
    "<同 id>": { "type": "noul",  "noul": 0.95 },
    "<同 id>": { "type": "choice", "choice": "billing",
                 "probabilities": { "billing": 0.88, "technical": 0.12 },
                 "confidence": 0.81 },
    "<同 id>": { "type": "score", "score": 1.05,
                 "legend": { "0": "Calm", "1": "Frustrated", "2": "Very angry" },
                 "probabilities": { "0": 0.0, "1": 0.95, "2": 0.05 },
                 "confidence": 0.92 }
  },
  "usage": { "input_tokens": 318, "output_tokens": 34 }
}
```

- `choice` / `score` 的答案带 `confidence`（0–1，由概率分布导出）；**`noul` 只有概率**。
- `score` 是**概率加权**的档位值，可以落在两档之间（如 1.05）。
- 响应里的 `model` 是**带版本的真实模型 id**（如 `jev-1.13.0`）——要记日志。

## 模型与限制（Jev 1.13）

| 项 | 值 |
| --- | --- |
| 模型 id | `jev-1.13.0`；别名 `jev-latest` / `jev-preview` |
| 价格 | $42 / Btok（**只按输入 token 计费，输出免费**） |
| 速率 | 100K tokens/s、80 requests/s（动态调整中，超限返回 429） |
| 上下文 | 单请求 64k tokens；其中 state + 最长的那一个问题 ≤ 32k |
| 输入 | 仅文本（string / JSON object / array）；图像音频需先转文本 |
| 语言 | 英文最佳；**CJK 可用但不是同等质量**——用我们自己的中文材料先测 |
| 数据 | **不用客户请求/响应训练**；企业版有零留存（ZDR），详见 Legal |

## 映射到本仓库（接入时怎么用）

| 我们的概念 | Jev 用法 | 说明 |
| --- | --- | --- |
| `DecisionModel::decide(kind, state)` | **每个候选一个问题**（`score`，档位如"不相关/部分相关/直接相关"），一次请求批量发出 | `score / (档数-1)` → 我们的 `score`；`confidence` 直接用；档位映射到 `DecisionChoice` |
| `DecisionModel::judge_conflict(query, cands)` | 一个 `noul`：「这些材料是否互相矛盾（结论对立）？」 | `noul ≥ 0.5` 判冲突；`noul` 无 confidence，理由里带上概率 |
| `DecisionResult.reason` | Jev 不返回理由 | 我们自己拼（`"Jev noul=0.95"` / `"Jev score=1.05 (confidence 0.92)"`） |
| 决策链路 | 作为**新的决策档**参与 D2 的 `DecisionEngine` | 走 `ResearchService::with_decision_model` 注入；A/B 用 `decisionTrace` 出对比表 |
| 闸门（C2） | Jev 是在线端点 ⇒ 需**独立授权** + 可选的逐次确认 | 复用 `ensure_remote_model_allowed` 的同款结构，但**不能用同一个 key/授权** |

## 接入前要落实的点

1. **端点白名单**：现有 `normalize_remote_base_url` 只放行 `https` 公网域名
   （`localhost` 例外；裸 IP / 内网单标签主机名会被拒）。`api.typesafe.ai` 满足；
   若要自建/代理端点，需要单独的安全口径决策。
2. **契约探针**：先发一条最小 `noul` 请求验证形状（`ai_jev_probe`，只读、不碰知识库）。
3. **凭证**：Key 存应用数据目录（`ai-credentials.json` 同款），**只写不读**，读路径只回布尔。
4. **成本提示**：按输入 token 计费，报告/设置里要能看到用量（响应带 `usage`）。
