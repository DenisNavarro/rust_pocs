#![warn(clippy::nursery, clippy::pedantic)]

//! Write a Ninja build file in stdout

use std::fs;
use std::io;
use std::iter;

use anyhow::anyhow;
use glob::glob;
use home::home_dir; // std::env::home_dir is deprecated since Rust 1.29.0.
use serde::Deserialize;

use ninja_bootstrap::rule;

fn main() -> anyhow::Result<()> {
    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let projects = toml::from_str::<CargoToml>(&cargo_toml)?.workspace.members;
    let home_path = home_dir().ok_or_else(|| anyhow!("failed to get the home directory path"))?;
    let bin_path = home_path.join("bin");
    let mut out = io::stdout().lock();
    rule("create_directory").command("mkdir -p -- $out").dump_rule(&mut out)?;
    rule("create_directory").output_unix_paths([bin_path.clone()]).dump_build(&mut out)?;
    rule("fmt").command("cargo fmt -p $project && touch $out").dump_rule(&mut out)?;
    rule("clippy")
        .command("cargo clippy -p $project -- -D warnings && touch $out")
        .dump_rule(&mut out)?;
    rule("test").command("cargo test -p $project && touch $out").dump_rule(&mut out)?;
    rule("release")
        .command("cargo build --release -p $project && touch $out")
        .dump_rule(&mut out)?;
    rule("copy").command("cp -- $in $out").dump_rule(&mut out)?;
    for project in projects.iter().map(String::as_str) {
        let fmt_ninjatarget = format!("{project}/fmt.ninjatarget");
        let clippy_ninjatarget = format!("{project}/clippy.ninjatarget");
        let test_ninjatarget = format!("{project}/test.ninjatarget");
        rule("fmt")
            .outputs([fmt_ninjatarget.clone()])
            .input_path_results(
                iter::once(Ok("rustfmt.toml".into()))
                    .chain(glob(&format!("{project}/src/**/*.rs")).unwrap()),
            )
            .variable("project", project)
            .dump_build(&mut out)?;
        rule("clippy")
            .outputs([clippy_ninjatarget.clone()])
            .inputs([fmt_ninjatarget.clone()])
            .variable("project", project)
            .dump_build(&mut out)?;
        rule("test")
            .outputs([test_ninjatarget.clone()])
            .inputs([fmt_ninjatarget.clone()])
            .variable("project", project)
            .dump_build(&mut out)?;
        if project != "ninja_bootstrap" {
            let release_path = format!("target/release/{project}");
            rule("release")
                .outputs([release_path.clone()])
                .inputs([format!("{project}/Cargo.toml"), fmt_ninjatarget])
                .variable("project", project)
                .dump_build(&mut out)?;
            rule("copy")
                .output_unix_paths([bin_path.join(project)])
                .inputs([release_path])
                .implicit_dependencies([clippy_ninjatarget, test_ninjatarget])
                .order_only_dependency_unix_paths([bin_path.clone()])
                .dump_build(&mut out)?;
        }
    }
    rule("phony")
        .outputs(["fmt"])
        .inputs(projects.iter().map(|project| format!("{project}/fmt.ninjatarget")))
        .dump_build(&mut out)?;
    rule("phony")
        .outputs(["check"])
        .inputs(projects.iter().flat_map(|project| {
            [format!("{project}/clippy.ninjatarget"), format!("{project}/test.ninjatarget")]
        }))
        .dump_build(&mut out)?;
    Ok(())
}

#[derive(Deserialize)]
struct CargoToml {
    workspace: Workspace,
}

#[derive(Deserialize)]
struct Workspace {
    members: Vec<String>,
}
