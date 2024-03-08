use crate::args::coverage_args::CargoCoverageArgs;
use crate::KaniSession;
use crate::project;
use crate::harness_runner;
use anyhow::Result;
use tracing::debug;

// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
pub fn coverage_cargo(mut session: KaniSession, args: CargoCoverageArgs) -> Result<()> {
    session.args.coverage = true;
    let project = project::cargo_project(&session, false)?;
    let harnesses = session.determine_targets(&project.get_all_harnesses())?;
    debug!(n = harnesses.len(), ?harnesses, "coverage_cargo");

    // Verification
    let runner = harness_runner::HarnessRunner { sess: &session, project: &project };
    let results = runner.check_all_harnesses(&harnesses)?;
    Ok(())
    //let install = InstallType::new()?;
    //cargo_test(&install, args)
}