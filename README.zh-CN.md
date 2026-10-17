# SteleOFD

OFD（Open Fixed-layout Document，版式文档）的 Rust 实现，涵盖阅读、转换与签章验证。

[English](README.md) | [简体中文](README.zh-CN.md)

[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange)](https://www.rust-lang.org/)
[![Edition](https://img.shields.io/badge/edition-2024-blue)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)
[![Standard](https://img.shields.io/badge/standard-GB%2FT%2033190--2016-red)]()

SteleOFD 是 [GB/T 33190-2016《电子文件存储与交换格式 版式文档》](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=3AF6682D939116B6F5EED53D01A9DB5D)
的 Rust 实现。从解析到渲染全部自研，仓库里唯一的 C 库是 fontconfig——
由 reader 在 Linux 上经 `font-kit` 调用。

当前进度：原始规划已全部完成。26 个 crate 全部就位并通过测试，五种交付形态
（`cli` / `wasm` / `ffi` / `server` / `reader`）共用同一个 `core` 门面，内核有
可量化的性能预算把关。当前发布版本为 `0.1.1`：尚待完成的事项及其所属版本见
[`docs/Roadmap.md`](docs/Roadmap.md)，本版本提供了什么见 [`CHANGELOG`](CHANGELOG.md)。

## 特性

| 能力 | |
|---|---|
| 阅读渲染 | GB/T 33190 渲染，无损缩放，超大文档分片加载 |
| 格式转换 | OFD ↔ PDF / SVG / PNG，支持批量与合并；Office 与 OCR 仅在路线图中，尚未实现 |
| 电子签章 | 国密 SM2/SM3/SM4 及国际算法验签，符合 GB/T 38540-2020 |
| 多端交付 | 同一内核编译为 CLI、WASM、C ABI、HTTP 服务与桌面阅读器 |
| 国际化 | 基于 Fluent 的消息层，内置 en-US / zh-CN |

## 背景

OFD 和 PDF 都是版式文档，区别在于 OFD 是开放的 ZIP + XML 容器，国密算法与电子印章
写进了标准本身，且要求版面在任意软硬件环境下保持一致。因此它被用作电子公文、
电子证照、电子发票、电子档案的法定载体。

选 Rust 有两个原因。版式文档处理是 CPU 密集型任务，同时要在浏览器、服务端、桌面端
运行，零成本抽象让分层架构不必以性能为代价。另外 OFD 文件来源任意，解析器又是处理
不可信输入的那一环，`#![forbid(unsafe_code)]` 在这里格外重要。

## 项目结构

```
steleofd/
├── crates/
│   ├── steleofd-common        值类型、版本、规范常量
│   ├── steleofd-error         错误体系
│   ├── steleofd-io            输入输出抽象（文件 / 内存 / 切片）
│   ├── steleofd-model         OFD 数据模型
│   ├── steleofd-xml           XML 编解码
│   ├── steleofd-package       ZIP 容器读写与校验
│   ├── steleofd-i18n          国际化消息层（Fluent）
│   ├── steleofd-geometry      路径与仿射变换
│   ├── steleofd-font          字体注册、度量、整形
│   ├── steleofd-display       显示列表 IR
│   ├── steleofd-parser        OFD → 显示列表
│   ├── steleofd-render        渲染后端
│   ├── steleofd-crypto        国密算法与 X.509
│   ├── steleofd-sign          电子签章与验签
│   ├── steleofd-convert       格式互转
│   ├── steleofd-edit          文档编辑
│   ├── steleofd-search        全文检索
│   ├── steleofd-archive       归档合规校验
│   ├── steleofd-print         打印
│   ├── steleofd-core          门面层
│   ├── steleofd-reader        桌面阅读器（egui/eframe）
│   ├── steleofd-cli           命令行工具
│   ├── steleofd-wasm          WASM 绑定
│   ├── steleofd-ffi           C ABI 动态库
│   └── steleofd-server        HTTP 服务
├── locales/                   翻译资源（.ftl，引擎 + 阅读器各一套）
├── scripts/                   构建与校验脚本
└── docs/                      架构文档
```

## 设计要点

### 只解析一次，多方消费

`Content.xml` 是一棵嵌套模板树。如果每个输出端各自遍历，逻辑会重复 N 份，更麻烦的是
各端会逐渐走偏 —— 「导出和预览看到的不一样」通常就是这么来的。

所以这棵树只拍平一次，产出带绝对变换的指令序列，下游全部消费这一份：

```
Content.xml (嵌套模板树)
       │
       ▼  仅此处解析一次
DisplayList (拍平 + 绝对 CTM + 资源已解析)
       │
       ├──► render    raster
       ├──► print
       ├──► convert   PDF / SVG / PNG
       └──► search    文本提取
```

Office 与 OCR 转换在路线图中，尚未落地。

### 引擎层不放文案

错误类型是带参数的结构化枚举，不含面向用户的字符串：

```rust
// 任意语言都能渲染
FormatError::PageOutOfRange { index: 42, total: 100 }

// 硬编码文案会让引擎无法被 C ABI / JS / Python 复用
"页索引越界: 42 (共 100 页)"
```

第二行仅作对照，实际代码中不存在硬编码文案。

### 不可信输入

| 威胁 | 防护 |
|---|---|
| Zip Slip 路径穿越 | `PackageError::IllegalPath` |
| Zip Bomb | 条目数上限 + 解压体积上限（512 MB） |
| 超深嵌套打爆栈 | `DisplayListBuilder::max_depth`（默认 64） |
| 恶意文档挂死 | `RenderError::BudgetExceeded` 降级为部分渲染 |
| 编辑破坏签名 | `EditError::WouldInvalidateSignature` 强制确认 |

## 快速开始

需要 Rust 1.92+（Edition 2024），这一版本由工作区清单的 `rust-version` 声明；
CI 实际使用的版本写在 `.github/workflows/rust.yml` 里。是否可用，跑一次
`cargo check --workspace` 即可确认。

构建各 crate 不需要 C 工具链。Linux 上的 reader 是例外：`font-kit` 链接
fontconfig，机器上没有对应头文件时需安装 `libfontconfig1-dev` 与 `pkg-config`。

```bash
git clone https://github.com/emfox/SteleOFD.git
cd SteleOFD

cargo check --workspace          # 全工作区检查
cargo build --release            # 发布构建
cargo test --workspace           # 运行测试
```

### crates.io 下载卡住

`static.crates.io` 不可达时，cargo 可能长时间没有输出。配置镜像源即可：

```toml
# ~/.cargo/config.toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
```

之后可用 `cargo check --offline` 完全离线构建。

### 校验脚本

```bash
python3 scripts/check_i18n.py          # 校验翻译资源一致性
python3 scripts/check_language.py      # 校验英文规范与注释密度
python3 scripts/check_i18n.py --strict # 警告也视为失败
```

国际化校验比对两套资源：文件集合、消息键集合、变量占位符是否一致，以及各自
Rust 侧键列表与其 FTL 实际键是否匹配。引擎消息（`crates/steleofd-i18n/locales`，
73 键）与阅读器界面消息（`crates/steleofd-reader/locales`，32 键）分开维护 ——
界面文案归展示层自己所有，引擎不会命名它不拥有的控件。

### 发布

推送 `v*` 标签会为 `egui` 支持的各个平台构建阅读器，连同 C ABI 动态库与引擎自身的
WASM 绑定一并发布：

| 产物 | 平台 | 运行方式 |
|---|---|---|
| `steleofd-reader-linux`、`-macos`、`-windows.exe` | 桌面 | 直接运行 |
| `steleofd-reader-web.zip` | 浏览器 | 解压后用 HTTP 提供；`file://` 页面无法加载模块 |
| `steleofd-reader-android.apk` | Android 7.0+（arm64） | 侧载安装；用一次性调试密钥签名，不是应用商店上传包 |

iOS 是唯一没做的 `egui` 平台：它的应用必须先经过 macOS 上的签名与打包步骤，而本项目
不附带任何密钥。

构建产物**未签名**（项目中不存在签名密钥），因此每个 release 附带一份 `SHA256SUMS`
清单，作为下载内容唯一可校验的依据：

```bash
python3 scripts/release_checksums.py verify .
sha256sum -c SHA256SUMS          # 也可以完全不依赖本项目
```

## 规范

| 标准 | 内容 |
|---|---|
| [GB/T 33190-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=3AF6682D939116B6F5EED53D01A9DB5D) | 电子文件存储与交换格式 版式文档 |
| [GB/T 33476.1-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=419233A04D029066CF032CFACBC8713B) | 党政机关电子公文格式规范 第 1 部分：公文结构 |
| [GB/T 33476.2-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=5B4E730245FF2FAF909435EBE6A659D2) | 党政机关电子公文格式规范 第 2 部分：显现 |
| [GB/T 38540-2020](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=EBF1360C272E40E7A8B9B32ED0724AB1) | 信息安全技术 安全电子签章密码技术规范 |
| [GM/T 0099-2020](https://std.samr.gov.cn/hb/search/stdHBDetailed?id=C60C676BC220358AE05397BE0A0AFABB) | 开放式版式文档密码应用技术规范（签名 / 加密 / 完整性保护） |
| [GB/T 39362-2020](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=C6B230010286DC79C84CF8EB72DE3F97) | 党政机关电子公文归档规范 |

GB/T 33476 分为三个部分，本项目遵循的是第 1、2 部分。GM/T 0099-2020 由国家密码管理局发布，
属密码行业标准，因此在行业标准平台而非国家标准公开系统上查阅。

## 开发路线

- [x] **P0** 工程骨架：分层架构、错误体系、显示列表 IR、国际化层
- [x] **P1** OFD 结构解析（`model` / `xml` / `package` / `parser`）
- [x] **P2** 光栅渲染后端，打通「打开 → 显示」链路
- [x] **P3** 格式转换（OFD → PDF / PNG / SVG）
- [x] **P3b** 打印：任务编排与光栅化（`print`）
- [x] **P4b** `core` 门面：单一入口，按交付形态开关特性
- [x] **P4** 国密验签
- [x] **P5a** `steleofd` 命令行工具
- [x] **P5b** 面向浏览器的 WASM 绑定
- [x] **P5c** 面向 OA / ERP 集成的 C ABI 动态库（`ffi`）
- [x] **P5d** 面向 OA / ERP 后端的 HTTP 服务（`server`）
- [x] **P6** 文档编辑与撤销栈
- [x] **P7** 基于显示列表的全文检索（`search`）
- [x] **P8** 归档就绪度检查（`archive`）
- [x] **P9** GB/T 33190-2016 合规闭环：从标准正文重建附录 A 的 Schema 并在 CI 强制校验

26 个 crate 已全部就位，五种交付形态（`cli` / `wasm` / `ffi` / `server` /
`reader`）共用同一个 `core` 门面。后续规划见
[`docs/Roadmap.md`](docs/Roadmap.md)，每一项标注了已被哪个版本认领，未认领的记为
`unversioned`。
条款级覆盖情况见
[`compliance/gb-t-33190.toml`](compliance/gb-t-33190.toml)：共登记 74 条，其中
73 条为 `verified`，唯一未覆盖的 `8.1.6`（`PrintQuality`）带 `deviation` 说明 ——
该元素在附录 A 的 Schema 中并不存在，文档里没有任何可断言的字节。
字体支持矩阵见 [`docs/Fonts.md`](docs/Fonts.md)，与其他阅读器逐页对照的结果见
[`docs/Interop.md`](docs/Interop.md)，性能基线与跑法见
[`docs/Performance.md`](docs/Performance.md)。

## 贡献

欢迎提交 Issue 与 Pull Request。提交前请确保以下全部通过：

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
python3 scripts/check_i18n.py --strict
python3 scripts/check_language.py
```

新增代码需附带测试，且不得引入 `unsafe`（`steleofd-ffi` 因 C ABI 需要豁免）。
代码、注释、commit message 与 issue 一律使用英文，见 [docs/Conventions.md](docs/Conventions.md)。

## 许可证

[Apache-2.0](LICENSE)

## 相关资源

- [国家标准全文公开系统](https://openstd.samr.gov.cn/bzgk/gb/index)，可在线预览上述 GB/T 标准
- [行业标准信息服务平台](https://std.samr.gov.cn/hb/)，可查阅 GM/T 标准
- [GmSSL](https://github.com/guanzhi/GmSSL)，国密算法开源实现
