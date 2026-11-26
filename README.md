# 调律 TEMPER

一份可以弹的乐理手册，现在也是一个可以练、可以记录、可以分享的完整前后端系统。

原交付物是一个 573 KB 的单文件 HTML。本仓库保留它的十個章节、纸色 / 墨色 / 焦糖的设计 token、Web Audio 合成与钢琴 / 吉他双乐器约定，并把“看与听”扩展成“练与记”：

- **手册 `/`** — 原十章内容与交互，吉他采样改为首次切换时懒加载，新增练习入口与云端状态同步。
- **工坊 `/studio`** — 和弦、转位、五线谱、键盘；吉他把位由 C++ 引擎一次返回多个可按形状；4–8 小节进行可试听、保存、分享。
- **练习 `/practice`** — 音程、和弦、音阶、读谱、节奏五类；服务端出题与判分，答案不随题目下发，错题给出解释与重听。
- **档案 `/profile`** — 正确率、连对、薄弱标签、最近练习、清单进度、保存的进行、JSON 导出。
- **分享 `/s/{code}`** — 只包含调、速度、级数与乐器；可试听，也可带回调律工坊。

---

## 1. 技术栈

| 层 | 方案 | 说明 |
|---|---|---|
| 前端 | 原生 HTML/CSS/ES Modules | 无框架、无打包器；`web/` 直接由后端托管 |
| HTTP | Rust · axum 0.8 / tokio | 类型化路由、JSON 错误封装、请求追踪 |
| 乐理 | Rust · `temper-core` | 音名、拼写、音程、和弦、音阶、调号、进行 |
| 指法 / 音集 | C++20 · `temper_native` | 组合搜索、音集向量、张力、对称性 |
| 持久化 | SQLite · rusqlite（bundled） | WAL、连接池、预编译语句、事务 |
| 测试 | cargo test + node:test + 冒烟脚本 | 单测、API 集成测试、无头浏览器渲染检查 |

依赖只有 Rust crates 与 Node 标准库；前端没有 npm 运行时依赖。

---

## 2. 快速开始

### 环境

- Rust 1.82+（`x86_64-pc-windows-gnu` 或 MSVC 均可）
- C++20 编译器（Windows 推荐 MinGW-w64 GCC 13+）
- Node.js 20+（仅脚本与测试）
- Windows 上项目需放在 ASCII 路径（如 `C:\temper`），构建输出写入项目内 `.build/`

### 启动

```bash
npm run dev          # cargo run + web/ + data/temper.db
# 打开 http://127.0.0.1:8787
```

生产构建与启动：

```bash
npm run build        # release 二进制 + dist/ + 资源哈希清单
TEMPER_STATIC=dist cargo run --release -p temper-server
```

Windows 下若 `cargo` / `g++` 不在 PATH：

```bash
# 任一方式均可
set TEMPER_MINGW=C:\msys64\mingw64\bin
set PATH=C:\msys64\mingw64\bin;%PATH%
```

脚本会自动在 `C:\temper-tc\mingw64\bin`、`C:\msys64\mingw64\bin`、`%USERPROFILE%\.toolchains\mingw64\bin` 中寻找 GCC。构建输出默认写入项目内的 `.build/`；项目路径需保持在 ASCII 目录（如 `C:\temper`），因为 MinGW/GCC 无法在中文路径生成目标文件。可用 `TEMPER_BUILD_DIR` 覆盖输出目录。

### 验证

```bash
cargo test --workspace     # Rust 单测 + C++ 自检 + API 集成测试
npm test                   # 前端乐理函数
npm run smoke              # 启动真实二进制，跑完整 API + 无头 Edge 渲染
```

`npm run smoke` 会使用临时数据库、随机端口（默认 8791）并清理；无 Edge/Chrome 时自动跳过渲染检查并说明原因。

### 配置

| 变量 | 默认 | 作用 |
|---|---|---|
| `TEMPER_ADDR` | `127.0.0.1:8787` | 监听地址 |
| `TEMPER_DB` | `data/temper.db` | SQLite 文件 |
| `TEMPER_STATIC` | `web` | 静态目录（生产用 `dist`） |
| `TEMPER_LOG` | `temper_server=info` | tracing 过滤 |
| `TEMPER_CORS` | 关闭 | `1` 时允许跨域（仅开发） |

---

## 3. 架构

```
浏览器
  ├─ /            手册（原生 JS + Web Audio，采样懒加载）
  ├─ /studio      工坊（和弦 / 把位 / 进行 / 分享）
  ├─ /practice    练习（五类题型，逐题判分）
  ├─ /profile     档案（聚合与导出）
  └─ /s/{code}    分享
        │  fetch + X-Temper-Device
        ▼
Rust · temper-server (axum)
  ├─ routes     校验、错误封装、页面托管
  ├─ practice   出题 / 判分 / 节奏容差
  ├─ store      SQLite 连接池、事务、聚合
  └─ state      Arc<AppState>、设备标识、目录配置
        │  safe wrapper（唯一 unsafe 在 ffi.rs）
        ▼
Rust · temper-core（乐理、拼写、和弦匹配）
        │  extern "C"，平面 POD，调用方持有缓冲区
        ▼
C++20 · temper_native（把位搜索、音集分析）
```

### 集成边界

- C ABI 只有四个函数：`temper_abi_version`、`temper_native_selftest`、`temper_voicing_search`、`temper_analyze_pcs`。
- Rust 分配输出缓冲区并传入容量；C++ 最多写入 `out_cap` 个元素，用 `out_count` 返回实际数量，不保留指针。
- C++ 无堆分配、无全局可变状态、无异常跨边界；错误用整数码返回（`1` 参数、`2` 范围、`3` 空间不足）。
- ABI 版本在首次调用时校验，不一致直接返回“native core unavailable”，服务仍能启动但相关接口返回 503。

细节见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)。

---

## 4. API

所有接口以 `/api` 开头。除健康检查与乐理查询外，需要请求头 `X-Temper-Device`（6–64 位字母、数字、`-`、`_`），服务端首次见到时自动建用户。

| 方法与路径 | 作用 |
|---|---|
| `GET /api/health` | 服务、C++ 引擎、SQLite 状态 |
| `GET /api/theory/catalog` | 和弦、音阶、调式、音程、进行、拍号、12 调 |
| `POST /api/theory/analyze` | `{notes:[midi]}` → 和弦匹配 + 音集向量 / 张力 / 对称性 |
| `GET /api/theory/voicings` | `root`, `quality`, `inversion`, `maxFret`, `maxSpan`, `minStrings`, `limit` |
| `GET /api/theory/chord` | 和弦音、转位、低音、符号 |
| `GET /api/theory/scale` | 音阶音与公式 |
| `GET /api/theory/key` | 调号、关系小调、正三和弦 |
| `GET /api/state` · `PUT` | 乐器、调、速度、预设、级数、声音 |
| `GET /api/progress` · `PUT` | 学习清单 |
| `GET /api/profile` | 统计、按类型、薄弱标签、最近练习、清单 |
| `POST /api/practice/sessions` | `{mode,difficulty,length}` → 题目（不含答案） |
| `GET /api/practice/sessions/{id}` | 会话状态与已答题的正确答案 |
| `POST /api/practice/sessions/{id}/answers` | `{questionId,answer?,tapsMs?,elapsedMs?}` |
| `POST /api/practice/sessions/{id}/finish` | 汇总：正确率、得分、按标签 |
| `GET /api/practice/history` | 最近练习 |
| `GET/POST /api/presets` · `DELETE /api/presets/{id}` | 自定义进行 |
| `POST /api/shares` · `GET /api/shares/{code}` | 分享与读取 |

错误统一为：

```json
{ "error": { "code": "validation_failed", "message": "…", "details": { "field": "tempo" }, "retryable": false } }
```

状态码：`400` 请求头 / JSON 不合法，`404` 不存在或不属于当前设备，`409` 重复作答 / 已结束，`422` 字段校验，`503` 数据库繁忙或原生引擎不可用，`500` 其他。

---

## 5. 数据模型

| 表 | 关键字段 | 用途 |
|---|---|---|
| `users` | `id`, `created_at`, `last_seen` | 设备标识，无密码 |
| `device_state` | `instrument`, `key_pc`, `tempo`, `preset_id`, `degrees`, `sound` | 手册 / 工坊共享状态 |
| `practice_sessions` | `mode`, `difficulty`, `total`, `answered`, `correct`, `score` | 一组练习 |
| `practice_questions` | `prompt`, `answer`, `tag`, `correct`, `elapsed_ms` | 题目与判分，答案只在作答后下发 |
| `progress_items` | `user_id`, `item_id`, `done` | 清单 |
| `custom_presets` | `name`, `degrees`, `key_pc`, `tempo` | 保存的进行 |
| `shares` | `code`, `title`, `payload`, `views` | 分享快照 |

`PRAGMA user_version` 管理迁移；当前 schema 版本 1。WAL + `synchronous=NORMAL` + `busy_timeout=5s`，连接数 `clamp(cores/2, 2, 8)`，每请求一条连接，写操作走事务与 `prepare_cached`。

---

## 6. 并发与资源

- **HTTP**：tokio 多线程运行时；`Arc<AppState>` 只读共享；无每请求全局锁。
- **SQLite**：连接池 + RAII 归还；读不阻塞写；聚合查询在数据库内完成。
- **C++**：纯函数、可重入；栈上候选表（`array<int32,25>` × 8 弦）与固定结果缓冲；单次调用 400 万节点预算防止退化输入拖垮请求。
- **音频**：懒创建 `AudioContext`；声部上限 40，超限统一停止；钢琴为真实三角钢琴采样，页面加载时经 `OfflineAudioContext` 解码（不会在手势前打开音频上下文），同时发声上限 48；吉他采样只在切到吉他时 `fetch + decode`，失败会显示“采样不可用”而不是静默换音色。
- **前端**：手册状态与清单 debounce 写入；页面切换不重建音频上下文。

---

## 7. 保留的约定

| 约定 | 状态 |
|---|---|
| 纸色 `#F3F0E7` / 墨色 `#17150F` / 焦糖 `#9C5227` / 深绿 `#3D5F52` | 保留，新页面复用同一 token |
| Instrument Serif + Noto Serif SC / Inter / IBM Plex Mono | 保留 |
| 十章结构与锚点 `#pitch` … `#path` | 保留，ID 未改 |
| 进行 ID `canon` `p4536251` `p1645` `p1564` `p4536` `cad` `desc` | 前后端一致 |
| 和弦 ID `maj` `min` `dim` `aug` `sus2` `sus4` `maj7` `m7` `dom7` `m7b5` `dim7` | 前后端一致 |
| 标准调弦 `[40,45,50,55,59,64]`、24 品 | 保留 |
| 声音必须由用户手势启动 | 保留（`Sound.resume()` 在点击后调用） |
| 不引入第二种强调色或紫色 | 保留 |

与原单文件的差异：采样从内联 base64 移到 `web/assets/guitar-samples.js`（首次切吉他时加载）；新增页面、API 与服务端；清单与状态可持久化。

---

## 8. 目录

```
Cargo.toml                 workspace
crates/temper-core/        Rust 乐理 + C++ 桥
  native/                  C++20 核心（C ABI）
crates/temper-server/      axum 服务、存储、练习引擎、测试
web/                       可直接托管的静态前端
  index.html               手册（由 scripts/migrate-handbook.py 生成）
  studio.html practice.html profile.html share.html
  css/app.css  js/*.js  assets/guitar-samples.js  assets/piano-samples.js
scripts/                   dev / build / smoke / 迁移
tests/frontend/            node:test
docs/ARCHITECTURE.md
```

---

## 9. 资产与许可

| 资产 | 来源 | 许可 |
|---|---|---|
| 吉他采样 | FreePats《Spanish Classical Guitar》，roberto@zenvoid.org | CC0 1.0 |
| 钢琴采样 | 《Salamander Grand Piano》，Alexander Holm（[tonejs.github.io/audio/salamander](https://tonejs.github.io/audio/salamander/)） | CC BY 3.0 |
| 字体 | Google Fonts：Instrument Serif、Noto Serif SC、Inter、IBM Plex Mono | SIL OFL |
| 其余图形与代码 | 本项目 | MIT |

吉他采样处理沿用原项目：2.6 s、RMS 归一化、软限幅、MP3 96 kbps 单声道 44.1 kHz，逐条实测音分并在播放时校正。正式发行建议随包保留原始 `cc0.txt`。

钢琴采样取自 C2–C7 每个大三度一条（共 16 条）：裁去开头静音、截取 3.2 s 并淡出、按首秒 RMS 平衡（保留四成原录音的音区响度差）、软限幅、MP3 96 kbps 单声道 44.1 kHz，数据在 `web/assets/piano-samples.js`（约 830 KB），页面加载时解码，不占用交互时机。原录音是拉伸调律，逐条用谱峰测出音分偏差（低音区约 −11、高音区约 +24），播放时校正回十二平均律，A4 = 440 Hz。采样未就绪或加载失败时退回柔和的合成音。
