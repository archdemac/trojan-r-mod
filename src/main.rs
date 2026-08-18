#![forbid(unsafe_code)]
// zig 链接器会输出 "ignoring deprecated linker optimization setting" 的提示，
// 这是 zig 工具链自身的兼容性提示，与项目代码无关，在此统一屏蔽。
#![allow(linker_messages)]

use clap::{Arg, Command};

mod error;
mod protocol;
mod proxy;

#[tokio::main]
async fn main() {
    let matches = Command::new("trojan-r")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .required(true)
                .value_parser(clap::value_parser!(String))
                .help(".toml config file name"),
        )
        .author("Developed by @p4gefau1t (Page Fault)")
        .about("An unidentifiable mechanism that helps you bypass GFW")
        .get_matches();
    let filename = matches.get_one::<String>("config").unwrap().to_string();
    if let Err(e) = proxy::launch_from_config_filename(filename).await {
        println!("failed to launch proxy: {}", e);
    }
}
