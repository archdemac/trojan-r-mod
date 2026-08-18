# Trojan-R

高性能的 Trojan 代理，使用 Rust 实现。为嵌入式设备或低性能机器设计。R 意为 **R**ust / **R**apid。

**Trojan-R 目前为实验性项目，仍处于重度开发中，协议、接口和配置文件格式均可能改变，请勿用于任何生产环境。**

## 特性

- 极致性能

    牺牲部分灵活性，采用激进的性能优化策略以极力减少不必要的开销。采用[更高效](https://jbp.io/2019/07/01/rustls-vs-openssl-performance.html)的 `rustls` （相较 openssl）建立 TLS 隧道以提升加解密的性能表现。

    使用 tokio 异步运行时，允许 `Trojan-R` 同时使用所有 CPU 核心，保证低时延和高效的吞吐能力。

    > 需要更多 benchmark 数据和更多优化

- 低内存占用

    Rust 无 GC 机制，内存占用可被预计。简化的握手和连接流程，仅使用极少的堆内存和复制。

    > 需要更多 benchmark 数据和更多优化

- 简易配置

    使用 toml 格式配置，仅需数行配置即可启动完整客户端或服务器。

- 内存安全

    使用 Rust 语言实现，可证明的内存安全性。在语法层面保证所有内存操作安全可靠。无竞争条件，无悬挂指针，无 UAF，无 Double Free。

- 密码学安全

    使用 `rustls` 建立 TLS 加密安全信道，过时的或不安全的密码学套件[均被禁用](https://docs.rs/rustls/0.18.1/rustls/#non-features)。`Trojan-R` 强制开启服务器证书校验以防止中间人攻击。

- 隐蔽传输

    `Trojan-R` 使用 TLS 建立代理隧道，难以从正常 TLS 流量中被区分。支持协议回落，在遭到主动探测时将与普通 TLS 服务器表现一致。

- 跨平台支持

    `Trojan-R` 可被交叉编译，支持 Android， Linux，Windows 和 MacOS 等操作系统，以及 x86，x86_64，armv7，aarch64 等硬件平台。

## 非特性

由于与项目的设计原则冲突，下列特性不计划实现

- 统计功能，包括 API 和数据库对接等

- 路由功能

- 用户自定义协议栈

- 透明代理

如果需要实现上述功能，请使用其他类似工具与 `Trojan-R` 组合实现。

## 设计原则

- 安全性

    `Trojan-R` 不涉及底层操作，且目前的性能瓶颈与其无关，无使用 unsafe rust 的必要。协议回落和 TLS 配置等安全敏感代码经过仔细考虑和审计，同时也欢迎更多来自开源社区的安全审计。

    目前 `Trojan-R` 使用 `#![forbid(unsafe_code)]` 禁用 unsafe rust。如未来有必要使用 unsafe rust 时，必须经过严格审计和测试。

- 使用静态分发而非动态分发

    协议实现使用统一的 trait。协议嵌套使用静态分发，以保证嵌套协议栈的函数调用关系在编译时被确定，使编译器可以进行内联和更好的优化。

- 低内存分配

    减少热点代码的内存分配，用引用替换复制，以实现更高的性能和更低的内存开销。

- 简洁

    保持最简洁干净的实现，以保证最低的代码复杂度，尽可能少的性能开销，并增加可靠性和减少攻击面。

## 部署和使用

`Trojan-R` 使用 toml 进行配置，参考 `config` 文件夹下配置文件。

## 编译

```shell
cargo build --release
```

交叉编译基于 `cross` 完成，编译前请确认已经安装 `cross` (`cargo install cross`)

```shell
make armv7-unknown-linux-musleabihf
```

编译默认开启链接时优化，以提升性能并减小可执行文件体积，因此编译耗时可能较其他项目更长。

编译完成后可以使用 `strip` 去除调试符号表以减少文件体积。

## TODOs

- [ ] 更完善的交互接口和文档

- [ ] 更多的单元测试和集成测试

- [ ] 性能调优

- [ ] 可复现的 benchmark 环境

- [ ] 实现 lib.rs 和导出函数

- [x] 分离客户端和服务端 features

- [ ] Github Actions

## 致谢

- [trojan](https://github.com/trojan-gfw/trojan)

- [shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust)

## 变更记录

### 依赖升级

- 升级 `tokio` 至 1.x（原 ~1.47）
- 升级 `tokio-rustls` 至 0.26（原 0.22），改用 `ring` 后端，避免 aws-lc-rs 在 musl 交叉编译时的 C 编译依赖
- 升级 `tokio-tungstenite` 至 0.30（原 0.14）
- 升级 `clap` 至 4（原 2.33）
- 升级 `toml` 至 1（原 0.5）
- 升级 `env_logger` 至 0.11（原 0.8）
- 升级 `sha2` 至 0.11（原 0.9）
- 升级 `webpki-roots` 至 1（原 0.21）
- 升级 `bytes`、`log`、`async-trait`、`serde`、`futures-*` 至最新大版本
- 移除 `webpki` 依赖（rustls 0.23 改用 `rustls-pki-types`）
- 新增 `rustls-pemfile` 依赖（rustls 0.23 将 pemfile 移出为独立 crate）

### 代码适配新 API

- `main.rs` 适配 clap 4：改用 `Command`、`Arg::new`、`value_parser`、`get_one`
- TLS 模块适配 rustls 0.23：改用 builder 模式（`builder_with_provider` + `with_no_client_auth` + `with_single_cert`）
- TLS 证书加载改用 `rustls-pemfile`（`certs` / `private_key`）
- TLS 域名校验改用 `ServerName`（替代已移除的 `DNSNameRef`）
- TLS 密码套件改用 `ALL_CIPHER_SUITES` 与 `CryptoProvider` 自定义配置
- `proxy/mod.rs` 适配 toml 1.0 错误类型显式转换
- websocket 模块适配 tokio-tungstenite 0.30：改用 `Bytes::copy_from_slice`

### 消灭警告

- 移除 `socks5/mod.rs` 未使用的 `u8`、`vec` 导入
- 修复 `UdpAssociateHeader.frag` 字段未使用：读取 UDP 头时校验 FRAG 字段
- 修复 59 个 clippy 警告（`needless_return`、`unnecessary_unwrap`、`io_other_error`、`to_string_in_format_args` 等）
- 屏蔽 zig 链接器兼容性提示（`#![allow(linker_messages)]`）

### Feature 门控

- 按 feature 门控模块：`direct`/`plaintext` 仅 server，`dokodemo` 仅 forward，`socks5` 仅 client
- 按 feature 门控子模块：各协议 `acceptor` 仅 server，`connector` 仅 client/forward
- 消除非当前 feature 下的 dead code 警告，同时保持 `full` 构建完整
- 将 `impl ProxyTcpStream for TcpStream` 移至共享的 `protocol/mod.rs`

### 数据部分读写修复

- trojan 首包 hash 读取改为循环累积直到读满或 EOF（原 `read` 可能部分读取导致误判）
- trojan UDP payload 读取用 `min` 限制长度，防止越界
- trojan UDP 写入改用 `write_all`，防止部分写入
- trojan fallback 转发改用 `write_all`，防止部分写入
- mux UDP 写入改用 `write_all`，防止部分写入
- mux 断言 `data.len() <= MAX_DATA_LEN`（原 `<` 在数据恰好等于上限时会 panic）
- socks5 UDP payload 拷贝用 `min` 限制长度，防止越界
- websocket 二进制消息长度判断改为 `<=`，修正边界处理

### 依赖缩减

- 合并 `futures-core` + `futures-util` 为仅 `futures-util`（其已重新导出 `ready`/`Stream`/`Future`）
- `tokio` 移除 `rt` feature（`rt-multi-thread` 已隐含）
- `clap` 精简 features：仅保留 `std`/`help`/`usage`/`error-context`，移除 `color`/`suggestions`
- `env_logger` 精简 features：仅保留 `auto-color`，移除 `humantime`/`regex`
- Cargo.lock 包数量由 147 减至 102
- 二进制体积减小约 22%–25%（native 2.7M→2.1M，x86_64 musl 3.6M→2.7M，aarch64 musl 3.0M→2.3M）

### 其他

- 新增 `.gitignore`，忽略 `target/` 目录
- release profile 的 `lto` 由 `true` 改为 `thin`
- 验证三个构建目标：`cargo zigbuild -r --target x86_64-unknown-linux-musl`、`cargo zigbuild -r --target aarch64-unknown-linux-musl`、`cargo b -r`
- 修复 `--version` 输出不跟随 Cargo.toml 版本号的问题：`main.rs` 改用 `env!("CARGO_PKG_VERSION")` 动态读取，替代硬编码的 `v0.1.0`
