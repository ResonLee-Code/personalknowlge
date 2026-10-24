# 决策模型 Key 显示与 401 诊断

## 现象

配置 Perplexity Decisions 后运行契约探针，服务端返回：

```text
HTTP 401（主机 api.perplexity.ai，模型 pplx-decider-v1.1-27b）
Invalid API key provided.
```

同时，已配置的在线模型 Key 输入框保存后只剩空输入框和“已配置 Key”标签，不容易确认
“只写不读”是否生效。

## 结论

- 客户端会发送 `Authorization: Bearer <decision key>`，请求已经到达
  `https://api.perplexity.ai/v1/decisions`；
- 服务端明确返回 401，因此当前保存值不能作为该端点的有效凭据使用；
- DeepSeek Key 与 Perplexity Key 相互独立，不能交叉使用；
- “已配置 Key”只表示本机凭据文件存在非空值，不表示服务商已接受该 Key。

真实 Key 没有在诊断、日志、测试或本文中读取或输出。

## 界面修复

1. 在线模型和决策模型的 Key 输入框在已配置时显示统一掩码占位与状态标签；
2. 掩码只作为 `placeholder`，不写入受控 `value`，真实 Key 仍不会回 IPC 或 DOM；
3. 决策模型补齐“模型基地址 / 模型名 / API Key”常驻标签与“已配置 Key”状态；
4. 空白输入时禁用保存，避免误把已有 Key 覆盖为空；
5. 401 后探针错误会保留在页内，并提供 Key 轮换、完整明文检查和端点检查步骤。

## 用户操作

1. 打开 <https://console.perplexity.ai> 创建或轮换专用 API Key；
2. 确认复制的是完整明文 Key，不带引号或额外空格，也不是界面上的掩码；
3. 在「设置 → AI → 决策与高级 → 决策模型」先点“清除”；
4. 地址保持 `https://api.perplexity.ai`，模型保持 `pplx-decider-v1.1-27b`；
5. 保存新 Key，确认状态为“已配置 Key”；
6. 保持决策模型已授权，重新运行契约探针。

若仍为 401，需要在 Perplexity 控制台核对 API 项目、Key 状态和账号的 API 权限/额度；
应用侧已确认不是缺少 Bearer 头或误用 DeepSeek Key 的代码路径问题。
