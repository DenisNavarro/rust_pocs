#![warn(clippy::nursery, clippy::pedantic)]

//! Write a Ninja build file in stdout

use anyhow::anyhow;
use glob::glob;
use home::home_dir; // std::env::home_dir is deprecated since Rust 1.29.0.
use ninja_bootstrap::rule_name;
use std::io;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let binary_names = ["ninja_bootstrap", "backup", "synchronize_backup"];
    let home_path = home_dir().ok_or_else(|| anyhow!("failed to get the home directory path"))?;
    let bin_path = home_path.join("bin");
    let copy_rule_name = rule_name("copy");
    let mut out = io::stdout().lock();
    copy_rule_name.command("cp -- $in $out").dump(&mut out)?;
    rule_name("create_directory")
        .command("mkdir -p -- $out")
        .build_output_paths([bin_path.clone()])
        .dump(&mut out)?;
    for &name in &binary_names {
        let fmt_ninjatarget = format!("{name}/fmt.ninjatarget");
        let clippy_ninjatarget = format!("{name}/clippy.ninjatarget");
        let test_ninjatarget = format!("{name}/test.ninjatarget");
        let rust_file_paths = glob(&format!("{name}/src/**/*.rs"))
            .unwrap()
            .collect::<Result<Vec<PathBuf>, _>>()?;
        rule_name(&format!("{name}_fmt"))
            .command(&format!("cargo fmt -p {name} && touch $out"))
            .build_outputs([fmt_ninjatarget.clone()])
            .input_paths(rust_file_paths)
            .dump(&mut out)?;
        rule_name(&format!("{name}_clippy"))
            .command(&format!(
                "cargo clippy -p {name} -- -D warnings && touch $out"
            ))
            .build_outputs([clippy_ninjatarget.clone()])
            .inputs([fmt_ninjatarget.clone()])
            .dump(&mut out)?;
        rule_name(&format!("{name}_test"))
            .command(&format!("cargo test -p {name} && touch $out"))
            .build_outputs([test_ninjatarget.clone()])
            .inputs([fmt_ninjatarget.clone()])
            .dump(&mut out)?;
        if name != "ninja_bootstrap" {
            let release_path = format!("target/release/{name}");
            rule_name(&format!("{name}_release"))
                .command(&format!("cargo build --release -p {name} && touch $out"))
                .build_outputs([release_path.clone()])
                .inputs([format!("{name}/Cargo.toml"), fmt_ninjatarget])
                .dump(&mut out)?;
            copy_rule_name
                .build_output_paths([bin_path.join(name)])
                .inputs([release_path])
                .implicit_dependencies([clippy_ninjatarget, test_ninjatarget])
                .order_only_dependency_paths([bin_path.clone()])
                .dump(&mut out)?;
        }
    }
    rule_name("phony")
        .build_outputs(["fmt"])
        .inputs(
            binary_names
                .iter()
                .map(|&name| format!("{name}/fmt.ninjatarget")),
        )
        .dump(&mut out)?;
    rule_name("phony")
        .build_outputs(["check"])
        .inputs(binary_names.iter().flat_map(|&name| {
            [
                format!("{name}/clippy.ninjatarget"),
                format!("{name}/test.ninjatarget"),
            ]
        }))
        .dump(&mut out)?;
    Ok(())
}
