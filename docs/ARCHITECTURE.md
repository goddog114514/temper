# TEMPER 架构说明

## 1. 为什么分成 Rust 与 C++

手册原本是纯前端。扩展成练习系统后，有两类计算值得放到服务端：

1. **正确性要求高、需要共享的乐理**——音名拼写、和弦构成、音阶、调号、进行。用 Rust 写，类型清楚，能被单测与 API 同时复用。
2. **组合量大的搜索**——给定一组音，枚举 6 弦 24 品上的可按形状，计算跨度、低音、覆盖、排序。用 C++20 写，固定数组、无堆分配、可预测延迟。

两者之间只保留一个窄的 C ABI，避免把 Rust 类型、C++ 容器或异常带过边界。

---

## 2. C ABI 契约

头文件：`crates/temper-core/native/temper_native.h`

```c
int32_t temper_abi_version(void);
int32_t temper_native_selftest(void);
int32_t temper_voicing_search(const int32_t *tuning, int32_t string_count,
                              const int32_t *chord_pcs, int32_t pc_count,
                              int32_t max_fret, int32_t max_span,
                              int32_t min_strings, int32_t max_results,
                              TemperVoicing *out, int32_t out_cap,
                              int32_t *out_count);
int32_t temper_analyze_pcs(const int32_t *pcs, int32_t count,
                           TemperAnalysis *out);
```

### 所有权与生命周期

| 对象 | 分配方 | 释放方 | 约定 |
|---|---|---|---|
| `tuning` / `chord_pcs` / `pcs` | Rust | Rust | 调用期间有效；C++ 不保留指针 |
| `out` / `out_count` | Rust | Rust | C++ 最多写 `out_cap` 个；`out_count <= out_cap` |
| `TemperVoicing` / `TemperAnalysis` | 值类型（POD） | 栈 | 无指针字段、无析构 |

C++ 不做动态分配：候选品格使用 `std::array<std::array<int32_t, 25>, 8>`，结果直接写入 Rust 提供的缓冲区。因此没有跨语言的内存释放问题，也没有分配器不匹配问题。

### 错误模型

| 码 | 含义 | Rust 映射 |
|---|---|---|
| 0 | 成功 | `Ok` |
| 1 | 参数为空 | `NativeError::Invalid` |
| 2 | 参数越界 | `NativeError::Invalid` |
| 3 | 输出空间不足 | `NativeError::Status` |
| 4 | 内部错误 | `NativeError::Status` |

C++ 编译时关闭异常（`-fno-exceptions`），边界内不会抛出。Rust 侧唯一含 `unsafe` 的文件是 `src/ffi.rs`；`src/native.rs` 负责校验入参、分配缓冲区、把 `out_count` 截断到容量内，再转换成 `serde` 结构。

### ABI 校验

`native::ensure_abi()` 用 `OnceLock` 只校验一次：

- 头文件版本 `ABI_VERSION = 1`
- 库返回 `temper_abi_version()`
- 不一致 → `NativeError::Unavailable` → API 返回 `503`，健康检查 `native.ok = false`

`temper_native_selftest()` 在启动时跑一遍：C 大三和弦的音集向量、Cmaj7 的把位数量、非法输入不崩溃。Rust 测试也会调用它，避免“只编译不验证”。

---

## 3. 把位搜索

输入：`tuning`（低到高 MIDI）、`chord_pcs`（第一个是根音）、`max_fret`、`max_span`、`min_strings`、`max_results`。

流程：

1. 归一化音级集合，建立 12 位布尔表；根音单独记录，用于低音加分。
2. 为每根弦列出 0..`max_fret` 内属于和弦音的品格（升序）。
3. 枚举连续弦区间 `[lo, hi]`，长度至少 `min_strings`；要求每根弦都有候选。
4. 以最低弦的候选品格作为锚点，把其余弦限制在 `[anchor, anchor+max_span]`（空弦单独保留），递归下降，边搜边用当前最高/最低按弦品格剪枝。
5. 叶子节点检查：覆盖全部和弦音、跨度、按弦数；计算分数：
   `40 + 音数×14 + 根音低音 32 − 跨度×5 − 最高品×2 + 空弦×3`
6. 结果缓冲固定为 `min(max_results, 512)`；满了以后用一次线性扫描替换最差项；最后插入排序（N 很小），去重。

复杂度与保护：

- 典型四音和弦、跨度 4：每弦约 4–6 个候选，单区间 < 5 万节点。
- 最坏情况（半音簇）可能爆炸，因此设 `kEnumBudget = 4,000,000` 节点上限；超限返回已找到的结果，延迟可控。
- 无锁：函数无全局可变状态，多线程并发调用安全；Rust 侧测试用 8 线程 × 40 次验证结果稳定。

`temper_analyze_pcs` 计算：

- 基数、`ic1..ic6` 音程向量
- 移调对称性（存在 t 使集合映射到自身）
- 最小覆盖弧（12 − 最大环状间隔）
- 张力：按音程类别加权（小二 / 三全音权重更高），归一到 0–100

---

## 4. Rust 服务

### 模块

| 模块 | 职责 |
|---|---|
| `theory` | 音名、拼写、和弦音、转位、音阶、调式、调号、进行、和弦识别 |
| `native` / `ffi` | C++ 安全包装与 ABI |
| `routes` | 路由、请求校验、JSON 形状、页面托管 |
| `practice` | 出题、答案生成、判分、节奏容差 |
| `store` | 连接池、迁移、CRUD、聚合 |
| `state` | 配置、`Arc<AppState>`、`Device` 提取器 |
| `error` | `AppError` → 统一错误信封；`ApiJson` 替换 axum 默认拒绝 |

### 请求路径

```
Device 提取器（校验头）
  → touch_user（UPSERT，记录 last_seen）
  → 业务校验（范围 / 长度 / 归属）
  → store（事务 / 预编译语句）
  → JSON
```

归属检查失败返回 `404` 而不是 `403`，避免泄露“这个 ID 存在但不属于你”。

### 数据库

- 连接池：`Mutex<Vec<Connection>> + Condvar`，上限 `clamp(cores/2, 2, 8)`，等待 5 秒后返回 503；RAII `PoolGuard` 归还连接。
- 初始化：`journal_mode=WAL`、`synchronous=NORMAL`、`foreign_keys=ON`、`temp_store=MEMORY`、`cache_size=-8000`。
- 迁移：`PRAGMA user_version`，当前 1；`CREATE TABLE IF NOT EXISTS` + 索引，整体在一个事务里。
- 语句：`prepare_cached` 复用；批量写入（题目、清单）使用事务。
- 时间：SQLite `strftime('%Y-%m-%dT%H:%M:%fZ','now')`；连续天数在 Rust 里用 civil-date 算法计算，不引入日期库。

### 练习引擎

| 模式 | 出题 | 判分 |
|---|---|---|
| `interval` | 13 类音程，随机根音，和声/旋律 | 选项 ID 对比 |
| `chord` | 11 种性质，根音 0–11 | 选项 ID 对比 |
| `scale` | 5 种音阶，主音 0–11 | 选项 ID 对比 |
| `sight` | 高/低音谱表，MIDI 范围随难度 | 音名对比 |
| `rhythm` | 拍号 + 八分网格 + 重音型 | 最近拍点匹配，容差 140/100/70 ms，命中 ≥80% |

答案保存在 `practice_questions.answer`，只在作答后返回。重复作答返回 `409`。得分 = 答对 10 + 速度加分（<1.5 s 加 5，<3 s 加 3，否则 1）。

---

## 5. 前端

| 文件 | 作用 |
|---|---|
| `web/index.html` | 原手册，保留内联 CSS/JS；由迁移脚本生成 |
| `web/js/handbook.js` | 健康检查、清单同步、乐器/声音状态同步 |
| `web/js/app.js` | API 客户端、设备标识、音频、五线谱 / 指板 / 键盘、chrome |
| `web/js/theory.js` | 纯函数乐理，可在 Node 中测试 |
| `web/js/studio.js` | 和弦、把位、进行、保存分享 |
| `web/js/practice.js` | 会话生命周期、五类题型、节奏采集 |
| `web/js/profile.js` | 聚合展示、清单、导出 |
| `web/js/share.js` | 分享读取与试听 |

设计 token 与手册一致；新页面不引入框架，避免为几个交互付出运行时成本。所有页面使用绝对资源路径，因此 `/studio`、`/s/{code}` 这类无扩展名路由不会解析错目录。

音频：懒创建 `AudioContext`；`tone()` 用两只振荡器 + 包络 + 低通，`chord()` 轻微错开起始时间，`stopAll()` 限制声部；节奏题使用 `AudioContext.currentTime` 调度点击，`performance.now()` 记录拍点，再换算成相对时间提交。

---

## 6. 构建与验证

```
scripts/env.mjs     解析工具链、项目内 .build/ 构建目录、TMP
scripts/build.mjs   cargo build --release + 静态资源引用校验 + dist/ + manifest
scripts/dev.mjs     cargo run，web/ + data/temper.db
scripts/smoke.mjs   真实二进制 + API 流程 + Edge --dump-dom
```

Windows 中文路径问题：MinGW 的 `ar` / `ld` 无法在非 ASCII 路径创建文件。项目根目录须为 ASCII 路径（如 `C:\temper`）；`env.mjs` 把 `CARGO_TARGET_DIR` 与 `TMP/TEMP` 统一指向项目内 `.build/target` 与 `.build/tmp`，并在 PATH 前插入检测到的 MinGW 与 Cargo 目录。可用 `TEMPER_BUILD_DIR` 覆盖构建根目录。

验证层次：

1. `cargo test --workspace`：乐理单测、C++ 自检、并发稳定性、SQLite 迁移、API 全流程。
2. `npm test`：前端纯函数（拼写、转位、进行、工具）。
3. `npm run smoke`：启动 release 二进制，跑健康检查、把位、分析、练习、分享、档案，再用 Edge 无头渲染五个页面并检查 DOM。

---

## 7. 已知边界

- 设备标识是本地生成的随机串，清缓存即换身份；没有账号与跨设备合并。
- 分享只保存快照，不跟随后续修改。
- 节奏判分以浏览器 `performance.now()` 为准，极端系统卡顿会影响精度；容差已按难度放宽。
- 手册仍保留内联 CSS/JS（约 165 KB），没有做进一步拆分；采样已外置。
- C++ 搜索上限 24 品、8 弦；当前 UI 固定标准调弦 6 弦。
- 未实现 MIDI 导出、麦克风跟练、双语与打印版式（原交接文档的 P2/P3 项）。
