use crate::args::coverage_args::CargoCoverageArgs;
use crate::KaniSession;
use crate::project;
use crate::harness_runner;
use anyhow::Result;
use tracing::debug;
use crate::session;
use std::process::Command;
use crate::OsString;
use crate::call_single_file::base_rustc_flags;
use crate::session::lib_playback_folder;
use crate::session::InstallType;
use crate::coverage::cov_mappings;

// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
pub fn coverage_cargo(mut session: KaniSession, args: CargoCoverageArgs) -> Result<()> {
    session.args.coverage = true;
    let project = project::cargo_project(&session, false)?;
    let harnesses = session.determine_targets(&project.get_all_harnesses())?;
    debug!(n = harnesses.len(), ?harnesses, "coverage_cargo");

    // Read coverage mappings
    let cov_mappings = cov_mappings::read_cov_mappings(&project);

    // Verification
    let runner = harness_runner::HarnessRunner { sess: &session, project: &project };
    let results = runner.check_all_harnesses(&harnesses)?;

    // More to come later
    Ok(())
}

/// Does `cargo run` with same toolchain and instrument flag to produce profraw file
fn cargo_prof(install: &InstallType, args: CargoCoverageArgs) -> Result<()> {
    let mut rustc_args = vec![];//base_rustc_flags(lib_playback_folder()?);
    let mut cargo_args: Vec<OsString> = vec!["run".into()];

    rustc_args.extend_from_slice(
        &["-C", "instrument-coverage", "--emit=mir"].map(OsString::from),
    );
    // rustc_args.extend_from_slice(
    //     &[
    //         "-C",
    //         "panic=abort",
    //         "-C",
    //         "symbol-mangling-version=v0",
    //         "-Z",
    //         "panic_abort_tests=yes",
    //     ]
    //     .map(OsString::from),
    // );
    // rustc_args.push("--kani-compiler".into());
    // if args.playback.common_opts.verbose() {
    //     cargo_args.push("-vv".into());
    // } else if args.playback.common_opts.quiet {
    //     cargo_args.push("--quiet".into())
    // }

    // if args.playback.message_format == MessageFormat::Json {
    //     cargo_args.push("--message-format=json".into());
    // }

    // if args.playback.only_codegen {
    //     cargo_args.push("--no-run".into());
    // }

    // cargo_args.append(&mut args.cargo.to_cargo_args());
    // cargo_args.append(&mut cargo_config_args());

    // // These have to be the last arguments to cargo test.
    // if !args.playback.test_args.is_empty() {
    //     cargo_args.push("--".into());
    //     cargo_args.extend(args.playback.test_args.iter().map(|arg| arg.into()));
    // }

    // Arguments that will only be passed to the target package.
    let mut cmd = Command::new("cargo");
    cmd.arg(session::toolchain_shorthand())
        .args(&cargo_args)
        // .env("RUSTC", &install.kani_compiler()?)
        // Use CARGO_ENCODED_RUSTFLAGS instead of RUSTFLAGS is preferred. See
        // https://doc.rust-lang.org/cargo/reference/environment-variables.html
        .env("CARGO_ENCODED_RUSTFLAGS", rustc_args.join(&OsString::from("\x1f")));
        // .env("CARGO_TERM_PROGRESS_WHEN", "never");

    session::run_terminal(&args.coverage.common_opts, cmd)?;
    Ok(())
}
