#![forbid(unsafe_code)]
#![warn(clippy::nursery, clippy::pedantic)]

//! Write a Ninja build file to stdout
//!
//! In the `bin_from_ninja` POC, `make build.ninja` calls
//! `RUST_LIB_BACKTRACE=1 target/debug/ninja_bootstrap > build.ninja`.
//!
//! `build.ninja` is in the `.gitignore`, but you can look at `example.ninja`, which is almost a
//! copy of `build.ninja`.

mod ninja_writer;

use std::fs;
use std::io::{self, Write};
use std::iter;
use std::path::PathBuf;

use anyhow::Context;
use glob::glob;
use home::home_dir; // std::env::home_dir is deprecated since Rust 1.29.0.
use serde::Deserialize;
use toml::value::Table;
use toml::Value;

use ninja_writer::{Config, NinjaWriter};

fn main() -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    let mut ninja_writer = NinjaWriter::new(Config, &mut out);
    write_rules(&mut ninja_writer)?;
    write_builds(&mut ninja_writer)
}

fn write_rules<W: Write>(ninja_writer: &mut NinjaWriter<W>) -> anyhow::Result<()> {
    ninja_writer.rule("create_directory")?.command("mkdir -p -- $out")?.end()?;
    ninja_writer.rule("fmt")?.command("cargo fmt -p $project && touch $out")?.end()?;
    ninja_writer
        .rule("clippy")?
        .command("cargo clippy --offline --frozen -p $project -- -D warnings && touch $out")?
        .end()?;
    ninja_writer
        .rule("test")?
        .command("cargo test --offline --frozen -p $project && touch $out")?
        .end()?;
    ninja_writer
        .rule("release")?
        .command("cargo build --offline --frozen --release -p $project && touch $out")?
        .end()?;
    ninja_writer.rule("copy")?.command("cp -- $in $out")?.end()?;
    Ok(())
}

fn write_builds<W: Write>(ninja_writer: &mut NinjaWriter<W>) -> anyhow::Result<()> {
    let cargo_toml = fs::read_to_string("Cargo.toml").context("failed to read Cargo.toml")?;
    let cargo_toml =
        toml::from_str::<CargoToml>(&cargo_toml).context("failed to parse Cargo.toml")?;
    let projects = cargo_toml.workspace.members;
    let home_path = home_dir().context("failed to get the home directory path")?;
    let bin_path = home_path.join("bin");
    ninja_writer.build()?.unix_output(&bin_path)?.rule("create_directory")?.end()?;
    for project in &projects {
        ninja_writer
            .build()?
            .output(format!("{project}/fmt.ninjatarget"))?
            .rule("fmt")?
            .input("rustfmt.toml")?
            .unix_input_results(glob(&format!("{project}/src/**/*.rs")).unwrap())?
            .variable_and_value("project", project)?
            .end()?;
        let local_dependencies = get_local_dependencies(project, &projects)?;
        let clippy_and_test_inputs: Vec<String> = iter::once(project)
            .chain(local_dependencies.normal_dependencies.iter())
            .chain(local_dependencies.dev_dependencies.iter())
            .map(|project| format!("{project}/fmt.ninjatarget"))
            .collect();
        ninja_writer
            .build()?
            .output(format!("{project}/clippy.ninjatarget"))?
            .rule("clippy")?
            .input("Cargo.lock")?
            .inputs(clippy_and_test_inputs.iter())?
            .variable_and_value("project", project)?
            .end()?;
        ninja_writer
            .build()?
            .output(format!("{project}/test.ninjatarget"))?
            .rule("test")?
            .input("Cargo.lock")?
            .inputs(clippy_and_test_inputs.iter())?
            .variable_and_value("project", project)?
            .end()?;
        if has_a_binary_to_deploy(project) {
            let release_path = format!("target/release/{project}");
            let project_and_normal_dependencies: Vec<String> =
                iter::once(project.into()).chain(local_dependencies.normal_dependencies).collect();
            ninja_writer
                .build()?
                .output(&release_path)?
                .rule("release")?
                .input("Cargo.lock")?
                .inputs(
                    project_and_normal_dependencies
                        .iter()
                        .map(|project| format!("{project}/fmt.ninjatarget")),
                )?
                .variable_and_value("project", project)?
                .end()?;
            ninja_writer
                .build()?
                .unix_output(bin_path.join(project))?
                .rule("copy")?
                .input(release_path)?
                .implicit_dependencies(project_and_normal_dependencies.iter().flat_map(
                    |project| {
                        [
                            format!("{project}/clippy.ninjatarget"),
                            format!("{project}/test.ninjatarget"),
                        ]
                    },
                ))?
                .unix_order_only_dependency(&bin_path)?
                .end()?;
        }
    }
    ninja_writer
        .build()?
        .output("fmt")?
        .rule("phony")?
        .inputs(projects.iter().map(|project| format!("{project}/fmt.ninjatarget")))?
        .end()?;
    ninja_writer
        .build()?
        .output("check")?
        .rule("phony")?
        .inputs(projects.iter().flat_map(|project| {
            [format!("{project}/clippy.ninjatarget"), format!("{project}/test.ninjatarget")]
        }))?
        .end()?;
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

fn has_a_binary_to_deploy(project: &str) -> bool {
    project != "ninja_bootstrap" && PathBuf::from(format!("{project}/src/main.rs")).is_file()
}

fn get_local_dependencies(
    project: &str,
    local_projects: &[String],
) -> anyhow::Result<Dependencies> {
    let cargo_toml_path = format!("{project}/Cargo.toml");
    (|| {
        let cargo_toml = fs::read_to_string(&cargo_toml_path).context("failed to read the file")?;
        let value = cargo_toml.parse::<Value>().context("invalid TOML")?;
        let table = value.as_table().with_context(|| format!("not a table: {value:?}"))?;
        let normal_dependencies = get_local_projects_from(table, "dependencies", local_projects)?;
        let dev_dependencies = get_local_projects_from(table, "dev-dependencies", local_projects)?;
        anyhow::Ok(Dependencies { normal_dependencies, dev_dependencies })
    })()
    .with_context(|| format!("error with {cargo_toml_path:?}"))
}

fn get_local_projects_from(
    table: &Table,
    key: &str,
    local_projects: &[String],
) -> anyhow::Result<Vec<String>> {
    match table.get(key) {
        Some(value) => {
            let table =
                value.as_table().with_context(|| format!("{key:?} is not a table: {value:?}"))?;
            Ok(table.keys().filter(|name| local_projects.contains(name)).cloned().collect())
        }
        None => Ok(vec![]),
    }
}

struct Dependencies {
    normal_dependencies: Vec<String>,
    dev_dependencies: Vec<String>,
}
