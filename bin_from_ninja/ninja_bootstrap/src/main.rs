#![warn(clippy::nursery, clippy::pedantic)]

//! Write a Ninja build file in stdout

use anyhow::anyhow;
use glob::glob;
use home::home_dir; // std::env::home_dir is deprecated since Rust 1.29.0.
use ninja_bootstrap::rule;
use std::io;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    // TODO: read the content of binary_names from the top-level Cargo.toml.
    let binary_names = ["ninja_bootstrap", "backup", "synchronize_backup"];
    let home_path = home_dir().ok_or_else(|| anyhow!("failed to get the home directory path"))?;
    let bin_path = home_path.join("bin");
    let mut out = io::stdout().lock();
    rule("create_directory")
        .command("mkdir -p -- $out")
        .dump_rule(&mut out)?;
    rule("create_directory")
        .output_paths([bin_path.clone()])
        .dump_build(&mut out)?;
    rule("fmt")
        .command("cargo fmt -p $bin_name && touch $out")
        .dump_rule(&mut out)?;
    rule("clippy")
        .command("cargo clippy -p $bin_name -- -D warnings && touch $out")
        .dump_rule(&mut out)?;
    rule("test")
        .command("cargo test -p $bin_name && touch $out")
        .dump_rule(&mut out)?;
    rule("release")
        .command("cargo build --release -p $bin_name && touch $out")
        .dump_rule(&mut out)?;
    rule("copy").command("cp -- $in $out").dump_rule(&mut out)?;
    for &bin_name in &binary_names {
        let fmt_ninjatarget = format!("{bin_name}/fmt.ninjatarget");
        let clippy_ninjatarget = format!("{bin_name}/clippy.ninjatarget");
        let test_ninjatarget = format!("{bin_name}/test.ninjatarget");
        let rust_file_paths = glob(&format!("{bin_name}/src/**/*.rs"))
            .unwrap()
            .collect::<Result<Vec<PathBuf>, _>>()?;
        rule("fmt")
            .outputs([fmt_ninjatarget.clone()])
            .input_paths(rust_file_paths)
            .variable("bin_name", bin_name)
            .dump_build(&mut out)?;
        rule("clippy")
            .outputs([clippy_ninjatarget.clone()])
            .inputs([fmt_ninjatarget.clone()])
            .variable("bin_name", bin_name)
            .dump_build(&mut out)?;
        rule("test")
            .outputs([test_ninjatarget.clone()])
            .inputs([fmt_ninjatarget.clone()])
            .variable("bin_name", bin_name)
            .dump_build(&mut out)?;
        if bin_name != "ninja_bootstrap" {
            let release_path = format!("target/release/{bin_name}");
            rule("release")
                .outputs([release_path.clone()])
                .inputs([format!("{bin_name}/Cargo.toml"), fmt_ninjatarget])
                .variable("bin_name", bin_name)
                .dump_build(&mut out)?;
            rule("copy")
                .output_paths([bin_path.join(bin_name)])
                .inputs([release_path])
                .implicit_dependencies([clippy_ninjatarget, test_ninjatarget])
                .order_only_dependency_paths([bin_path.clone()])
                .dump_build(&mut out)?;
        }
    }
    rule("phony")
        .outputs(["fmt"])
        .inputs(
            binary_names
                .iter()
                .map(|&bin_name| format!("{bin_name}/fmt.ninjatarget")),
        )
        .dump_build(&mut out)?;
    rule("phony")
        .outputs(["check"])
        .inputs(binary_names.iter().flat_map(|&bin_name| {
            [
                format!("{bin_name}/clippy.ninjatarget"),
                format!("{bin_name}/test.ninjatarget"),
            ]
        }))
        .dump_build(&mut out)?;
    Ok(())
}
