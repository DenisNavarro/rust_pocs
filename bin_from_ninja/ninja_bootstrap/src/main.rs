#![warn(clippy::nursery, clippy::pedantic)]

//! Write a Ninja build file in stdout

// The current code uses the builder pattern.
// After refactoring, the future code will probably use the typestate pattern instead.
// TODO: refactor the code!

use std::fs;
use std::io;
use std::iter;
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use glob::glob;
use home::home_dir; // std::env::home_dir is deprecated since Rust 1.29.0.
use serde::Deserialize;
use toml::value::Table;
use toml::Value;

use ninja_bootstrap::rule;

fn main() -> anyhow::Result<()> {
    let cargo_toml = fs::read_to_string("Cargo.toml").context("failed to read Cargo.toml")?;
    let cargo_toml =
        toml::from_str::<CargoToml>(&cargo_toml).context("failed to parse Cargo.toml")?;
    let projects = cargo_toml.workspace.members;
    let home_path = home_dir().ok_or_else(|| anyhow!("failed to get the home directory path"))?;
    let bin_path = home_path.join("bin");
    let mut out = io::stdout().lock();
    rule("create_directory").command("mkdir -p -- $out").dump_rule(&mut out)?;
    rule("fmt").command("cargo fmt -p $project && touch $out").dump_rule(&mut out)?;
    rule("clippy")
        .command("cargo clippy -p $project -- -D warnings && touch $out")
        .dump_rule(&mut out)?;
    rule("test").command("cargo test -p $project && touch $out").dump_rule(&mut out)?;
    rule("release")
        .command("cargo build --release -p $project && touch $out")
        .dump_rule(&mut out)?;
    rule("copy").command("cp -- $in $out").dump_rule(&mut out)?;
    rule("create_directory").output_unix_paths([bin_path.clone()]).dump_build(&mut out)?;
    for project in projects.iter().map(String::as_str) {
        rule("fmt")
            .outputs([format!("{project}/fmt.ninjatarget")])
            .input_unix_path_results(
                iter::once(Ok("rustfmt.toml".into()))
                    .chain(glob(&format!("{project}/src/**/*.rs")).unwrap()),
            )
            .variable("project", project)
            .dump_build(&mut out)?;
        let local_dependencies = get_local_dependencies(project, &projects)?;
        let clippy_and_test_inputs: Vec<String> = iter::once(project)
            .chain(local_dependencies.normal_dependencies.iter().map(String::as_str))
            .chain(local_dependencies.dev_dependencies.iter().map(String::as_str))
            .flat_map(|project| {
                [format!("{project}/fmt.ninjatarget"), format!("{project}/Cargo.toml")]
            })
            .collect();
        rule("clippy")
            .outputs([format!("{project}/clippy.ninjatarget")])
            .inputs(clippy_and_test_inputs.iter().cloned())
            .variable("project", project)
            .dump_build(&mut out)?;
        rule("test")
            .outputs([format!("{project}/test.ninjatarget")])
            .inputs(clippy_and_test_inputs)
            .variable("project", project)
            .dump_build(&mut out)?;
        if has_a_binary_to_deploy(project) {
            let release_path = format!("target/release/{project}");
            let project_and_normal_dependencies: Vec<String> =
                iter::once(project.into()).chain(local_dependencies.normal_dependencies).collect();
            rule("release")
                .outputs([release_path.clone()])
                .inputs(project_and_normal_dependencies.iter().flat_map(|project| {
                    [format!("{project}/fmt.ninjatarget"), format!("{project}/Cargo.toml")]
                }))
                .variable("project", project)
                .dump_build(&mut out)?;
            rule("copy")
                .output_unix_paths([bin_path.join(project)])
                .inputs([release_path])
                .implicit_dependencies(project_and_normal_dependencies.iter().flat_map(|project| {
                    [format!("{project}/clippy.ninjatarget"), format!("{project}/test.ninjatarget")]
                }))
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
        let table = value.as_table().ok_or_else(|| anyhow!("not a table: {value:?}"))?;
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
                value.as_table().ok_or_else(|| anyhow!("{key:?} is not a table: {value:?}"))?;
            Ok(table.keys().filter(|name| local_projects.contains(name)).cloned().collect())
        }
        None => Ok(vec![]),
    }
}

struct Dependencies {
    normal_dependencies: Vec<String>,
    dev_dependencies: Vec<String>,
}
