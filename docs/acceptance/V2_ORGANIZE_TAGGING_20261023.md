# V2 Organize 标签建议 200 题预检

## 范围

- 日期：2026-10-23
- 模型：`deepseek-flash`
- 入口：真实 `OrganizeService::generate_with_confirmation`
- 样本：20 个技术主题 × 10 篇，共 200 篇合成笔记
- 写入：只生成并持久化 pending 建议，不修改任何 Markdown 正文
- 评分：标签名或理由命中该主题正文中预先定义的合法标签词表，记为相关

该评测验证真实模型、建议持久化、标签 JSON 解析和 provider 选择；它不是用户真实知识库的
人工标注结果，不能替代最终人工验收。

## 结果

| 指标 | 结果 |
| --- | ---: |
| 用例 | 200 |
| 在线模型成功 | 200 |
| 降级到规则档 | 0 |
| 生成标签数 | 936 |
| 相关标签数 | 916 |
| 标签精确率 | **97.86%** |
| 至少包含一个相关标签的笔记 | 200 / 200 |
| 笔记召回率 | **100.00%** |
| 总耗时 | 747.7 s |

平均每篇生成 4.68 个标签。20 个未计分标签中既有真正过泛的“实践”标签，也有
“代码块处理”“本地编辑保护”等确实来自正文、但未预先列入词表的合理标签，因此该数字
同时包含两类误差，不能简单解释为纯误报率。

## 结论

- 本题集上的精确率和召回率均显著高于 80% 目标；
- 真实模型没有出现解析失败或静默降级；
- 生成阶段未修改笔记，符合“建议 → 确认 → 应用”边界；
- 真实用户知识库仍需要人工标注 200 条，尤其检查近义标签、过泛标签和已有标签复用。

## 复现

```bash
cred_file="$HOME/Library/Application Support/PersonalKnowledgeOS/ai-credentials.json"
PKOS_LIVE_TAG_CASES=200 \
PKOS_LIVE_REMOTE_KEY="$(jq -er '.remoteModelApiKey' "$cred_file")" \
  cargo test --offline --manifest-path src-tauri/Cargo.toml \
  --test organize_flow live_deepseek_200_tag_suggestions \
  -- --ignored --nocapture
```
