# 本机 SearXNG

这是 Personal Knowledge OS 的可选联网检索后端。服务只监听
`127.0.0.1:8080`，不会暴露到局域网；配置启用了 SearXNG 的 JSON 输出，
并继续使用官方默认引擎集合。

## 启动

```bash
bash scripts/searxng/start.sh
```

首次启动会生成权限为 `0600` 的 `infra/searxng/.env`，其中只有本机 SearXNG
会话密钥、端口和必要的容器出网代理，不会进入 Git。脚本会继承当前 macOS
系统代理的地址（适用于 Colima 容器），等待服务就绪并执行一次真实 JSON 搜索。

在「设置 → AI → 联网与增强 → 检索源地址」填写：

```text
http://127.0.0.1:8080/search?q={query}&format=json&count={limit}&language=zh-CN
```

然后在研究助手中开启联网并确认本次收集。SearXNG 会聚合其已启用的引擎；
若某个引擎被限流或验证码拦截，其他引擎仍可返回结果。

## 停止

```bash
bash scripts/searxng/stop.sh
```

停止容器不会删除搜索缓存和配置。需要更新镜像时，先停止服务，更新
`compose.yml` 中锁定的镜像摘要，再重新启动并执行真实检索验证。

## 安全边界

- 仅绑定回环地址，端口不暴露给局域网；
- 不启用公开实例模式和 bot limiter；
- JSON 输出只用于本应用的检索适配；
- 若宿主机使用其他代理，可在 `infra/searxng/.env` 设置
  `SEARXNG_PROXY_URL=http://host:port` 后重启服务；
- 应用仍会在每次研究收集前请求联网确认，不会因为服务已启动就自动外发。

