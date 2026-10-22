# V2 开发版 Windows / Linux 产物验收

## 范围与边界

- 代码基线：`49de15fbb17cbc468157ef2c477e901c4cffef9a`
- 版本：`1.3.7`（V2 开发线，不是 V2 正式发布号）
- 宿主：macOS 27.0.1 / arm64
- Windows：Zig 0.17.0 + rustup `x86_64-pc-windows-gnu` + NSIS 3
- Linux：Colima QEMU x86_64 隔离 VM，Ubuntu 22.04
- 本记录验证的是**当前源码生成的安装对象**。它不是 Developer ID / 公证 / Windows
  签名后的 Release 包，也不替代真实物理设备和版本升级验收。

## Windows

| 对象 | 大小 | SHA-256 |
| --- | ---: | --- |
| `Personal Knowledge OS_1.3.7_x64-setup.exe` | 4,611,461 B | `7233a98d4e49633824ddaa75d37bb3e8c6c42f85f6663cc6c51b5de100d50c17` |
| `Personal Knowledge OS_1.3.7_x64-portable.zip` | 6,140,186 B | `a7890cdc7649c0441182c7f3315b17af52be9c08e6d66f6958b5a340e1fa03cc` |
| `personal-knowledge-os.exe` | 13,423,104 B | `b0540b15ed28a500e3903da2a6205e60eae3d3daccf48b3f72d2f41ae2b591b2` |

验证结果：

- `verify-pe.py`：x86-64、Windows GUI、PE32+、资源节和产品名存在，引用 WebView2。
- NSIS 外壳为正常的 32 位 stub；`7zz t` 完整通过。
- 安装器内部包含 `personal-knowledge-os.exe`、`WebView2Loader.dll` 与 `uninstall.exe`。
- 从安装器提取的应用本体 SHA-256 与本次 release 构建完全一致。
- 便携 zip 完整通过 `unzip -t`；包内应用本体与 release 构建逐字节一致。
- 当前包未签名，尚未在真实 Windows 上执行 NSIS 安装、升级、卸载、ACL 和 SmartScreen。

## Linux

| 对象 | 大小 | SHA-256 |
| --- | ---: | --- |
| `Personal Knowledge OS_1.3.7_amd64.deb` | 8,731,578 B | `5da7b72cf7b11ed04a260ff0fb102724283b92ed5a432db1a67c93449df73dd0` |
| `Personal Knowledge OS-1.3.7-1.x86_64.rpm` | 8,732,677 B | `cac444993777e679aa114b33eb66a94acd2c475fd78d3f34f174e00f6ba7af2c` |
| `Personal Knowledge OS_1.3.7_amd64.AppImage` | 86,546,936 B | `48d10dd0778eadc3b1449bc32271a6c9e8b96b21087e99ad07ad0ad064b5483c` |

### deb

- `dpkg-deb --info / --contents` 通过：amd64、版本 1.3.7，包含可执行文件、desktop 文件
  与三种尺寸图标。
- 在 Ubuntu 22.04 x86_64 隔离环境中 `dpkg -i` 成功。
- 在 Xvfb 中实际启动到工作区首帧，耗时 `3393 ms`；界面显示 21 篇笔记、3 个分类、
  1 个标签和待复核卡片。
- 截图：[54-linux-ubuntu2204-vm.png](../screenshots/54-linux-ubuntu2204-vm.png)
- `dpkg -r` 后 `/usr/bin/personal-knowledge-os` 与 desktop 文件消失。
- 卸载前后知识库 23 个内容文件哈希完全一致；SQLite `integrity_check=ok`，仍为
  21 篇笔记 / 3 个分类 / 1 个标签。SQLite 主文件因正常 checkpoint 发生字节变化，
  但内容级数据和 Markdown 文件均未丢失。

### rpm

- `rpm -qpi / -qpl / -qpR` 通过：x86_64、版本 1.3.7，包含可执行文件、desktop 与图标。
- `rpm -K` 返回 `digests OK`。
- 当前验证环境是 Ubuntu；rpm 安装与升级仍需在 RPM 系真实发行版上人工执行。

### AppImage

- `--appimage-extract` 验证 `AppRun`、主程序、desktop 和图标存在。
- Tauri AppImage 所需的 `WebKitNetworkProcess`、`WebKitWebProcess` 与
  `libwebkit2gtkinjectedbundle.so` 已包含在 `usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/`，
  三个对象的 `RUNPATH` 均为 `$ORIGIN`。
- 使用 `APPIMAGE_EXTRACT_AND_RUN=1` 在 Xvfb 中实际启动到工作区首帧，耗时 `3303 ms`。
- Tauri / linuxdeploy 已生成完整 AppDir；容器的 AppImage 打包器无法联网下载 runtime，
  因而使用容器内已有 `/tmp/runtime-x86_64` 完成最终 AppImage 封装。
- 未在启用 FUSE 的物理桌面上双击启动；该步骤继续保留人工。

### 虚拟图形栈边界

- QEMU 虚拟显示默认的 WebKit 图形路径会触发
  `LLVM ERROR: 64-bit code requested on a subtarget that doesn't support it!`，
  因而软件层实启统一使用 `GALLIUM_DRIVER=softpipe`、
  `LIBGL_ALWAYS_SOFTWARE=1`、`WEBKIT_DISABLE_DMABUF_RENDERER=1` 与
  `WEBKIT_DISABLE_COMPOSITING_MODE=1`。
- 上述设置只用于隔离 VM 的软件渲染；物理 Linux 桌面直接启动、FUSE AppImage 双击和
  真实 GPU/WebKit 路径仍保留人工验收，不能用本轮结果替代。

## 仍保留的人工验收

- Windows：全新用户目录、中文空格路径、普通权限、升级、卸载、便携版、签名包、
  SmartScreen、凭据 ACL。
- Linux：Ubuntu 22.04 / 24.04 物理机 deb 安装、AppImage 直接双击启动、升级与卸载。
- macOS：Developer ID、公证、干净机器冷启动、真实自动更新和失败回滚。
- 三平台 Release 上传后的最终资产名、相对下载 URL、`SHA256SUMS-*` 与 `latest.json`。

权威人工清单：

- [V2 P0 人工清单](../V2_P0_MANUAL_ACCEPTANCE.md)
- [V2 P1 / P2 人工清单](../V2_P1_P2_MANUAL_ACCEPTANCE.md)
