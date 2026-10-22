# V2 Writing 2.0 真实模型五模式评测

## 范围

- 日期：2026-10-23
- 在线模型：`deepseek-flash`
- 本地模型：Ollama `qwen2.5:7b`
- 入口：应用真实 `WritingService::draft`
- 模式：大纲、续写、改写、缩短、扩写
- 素材：写作原则笔记 + 引用规范笔记；只生成草稿，不调用写入

本测试验证真实模型、引用编号、逐段引用结构和 provider 选择；不把自动结构通过等同于
人工语义质量通过。

## DeepSeek 在线模型

| 模式 | provider / tier | 段落 | 唯一来源 | 字符 | 模型耗时 | 墙钟耗时 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 大纲 | `remote.openai-compatible:deepseek-flash` / online | 15 | 3 | 312 | 17.881 s | 17.889 s |
| 续写 | 同上 / online | 2 | 1 | 154 | 4.890 s | 4.896 s |
| 改写 | 同上 / online | 5 | 1 | 103 | 4.304 s | 4.311 s |
| 缩短 | 同上 / online | 1 | 1 | 75 | 2.336 s | 2.343 s |
| 扩写 | 同上 / online | 4 | 2 | 268 | 10.449 s | 10.456 s |

5 个模式全部使用在线模型，所有段落均有引用，总墙钟耗时约 41.2 秒。

## 本地 Qwen2.5 7B

| 模式 | provider / tier | 段落 | 唯一来源 | 字符 | 模型耗时 | 墙钟耗时 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 大纲 | `local.ollama:qwen2.5:7b` / local | 3 | 3 | 191 | 5.656 s | 5.662 s |
| 续写 | 同上 / local | 1 | 1 | 74 | 1.177 s | 1.182 s |
| 改写 | 同上 / local | 1 | 1 | 75 | 1.259 s | 1.264 s |
| 缩短 | 同上 / local | 1 | 1 | 27 | 0.606 s | 0.611 s |
| 扩写 | 同上 / local | 3 | 1 | 278 | 3.668 s | 3.674 s |

5 个模式全部使用本地模型，所有段落均有引用，总墙钟耗时约 13.7 秒。

## 观察

- 本地模型明显更快，但大纲更粗，缩短结果以悬空“——”结尾；
- 本地续写和扩写会用泛化措辞解释预览、确认与本地优先，需要人工判断是否仍完全由材料支持；
- DeepSeek 大纲拆成 15 个较细段落，扩写会引入欢迎笔记作为背景来源，需要人工判断是否必要；
- 两个模型都通过结构化引用守卫，但这只证明编号可解析、段落有来源，不证明句意与来源完全一致。

## 仍待人工

- 评估结构、事实一致性、语气、重复和段落连贯性；
- 逐段核对句意确实由引用原文支撑，排除“引用了但内容无关”；
- 在 macOS / Windows 真机检查长文 diff、逐段选择、取消、确认和历史回滚；
- 抓包验证未确认在线外发时零请求。

## 复现

DeepSeek：

```bash
cred_file="$HOME/Library/Application Support/PersonalKnowledgeOS/ai-credentials.json"
PKOS_LIVE_SKIP_LOCAL=1 \
PKOS_LIVE_REMOTE_KEY="$(jq -er '.remoteModelApiKey' "$cred_file")" \
  cargo test --offline --manifest-path src-tauri/Cargo.toml \
  --test writing_flow live_models_cover_five_writing_modes \
  -- --ignored --nocapture
```

本地 Ollama：

```bash
cargo test --offline --manifest-path src-tauri/Cargo.toml \
  --test writing_flow live_models_cover_five_writing_modes \
  -- --ignored --nocapture
```
