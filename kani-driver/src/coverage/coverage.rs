use crate::args::coverage_args::CargoCoverageArgs;
use anyhow::Result;

// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
pub fn coverage_cargo(_args: CargoCoverageArgs) -> Result<()> {
    Ok(())
    //let install = InstallType::new()?;
    //cargo_test(&install, args)
}