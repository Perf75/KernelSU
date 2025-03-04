use anyhow::Result;
use const_format::concatcp;
use rust_embed::RustEmbed;
use std::path::Path;

use crate::{defs::BINARY_DIR, utils};

pub const RESETPROP_PATH: &str = concatcp!(BINARY_DIR, "resetprop");
pub const BUSYBOX_PATH: &str = concatcp!(BINARY_DIR, "busybox");

#[cfg(all(target_arch = "x86_64", target_os = "android"))]
#[derive(RustEmbed)]
#[folder = "bin/x86_64"]
struct Asset;

// IF NOT x86_64 ANDROID, ie. macos, linux, windows, always use aarch64
#[cfg(not(all(target_arch = "x86_64", target_os = "android")))]
#[derive(RustEmbed)]
#[folder = "bin/aarch64"]
struct Asset;

pub fn ensure_binaries(ignore_if_exist: bool) -> Result<()> {
    for file in Asset::iter() {
        if file == "ksuinit" || file.ends_with(".ko") {
            // don't extract ksuinit and kernel modules
            continue;
        }
        let asset = Asset::get(&file).ok_or(anyhow::anyhow!("asset not found: {}", file))?;
        utils::ensure_binary(format!("{BINARY_DIR}{file}"), &asset.data, ignore_if_exist)?
    }
    Ok(())
}