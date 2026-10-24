# 本机 SearXNG 联网检索验收

## 部署

- Docker 引擎：Colima，Docker Server `29.5.2`；
- SearXNG：`2026.10.24-ef0ec03da`，镜像锁定到审计过的 digest；
- Valkey：`9-alpine`，同样锁定 digest；
- 服务只发布到 `127.0.0.1:8080`，未暴露到局域网；
- JSON 输出在 `infra/searxng/core-config/settings.yml` 显式启用；
- SearXNG 会话密钥只存在于被 Git 忽略的本地 `.env`，权限 `0600`。

首次启动时，Docker 容器无法直接出网。启动脚本检测到 macOS 系统代理后，将 Colima
宿主网关代理注入容器；应用本身仍通过 `127.0.0.1` 访问 SearXNG，不读取代理凭据。

## 真实检索

查询 `Rust async runtime scheduler`：

```text
results=37
google cse=20
brave=23
unresponsive=duckduckgo(timeout)
```

引擎结果存在重叠，因此去重后的候选总数为 37。Google 和 Brave 均返回真实网页候选；
DuckDuckGo 超时没有阻断其他引擎。

应用后端闸门与解析链验证：

```text
PKOS_LIVE_SEARXNG_RESULTS=3
PKOS_LIVE_SEARXNG_SOURCE host=docs.searxng.org title=SearXNG Documentation
PKOS_LIVE_SEARXNG_SOURCE host=searxng.org title=SearXNG Documentation
PKOS_LIVE_SEARXNG_SOURCE host=docs.searxng.org title=User information
```

该测试走 `AiTools::web_search` 的完整 AI 开关、联网档位、本次确认、检索源请求和结果解析，
不是浏览器预览桩。

## 使用

```bash
bash scripts/searxng/start.sh
```

在「设置 → AI → 联网与增强」点击“填入本机 SearXNG”，然后点击“测试检索”。
研究收集仍会在每次运行前询问是否确认联网。
