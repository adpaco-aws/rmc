// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::Result;
// use std::ffi::OsString;
use std::path::Path;
use std::process::Child;
use crate::cbmc_output_parser::{call_loop, fmt_regular, postprocess_output, CbmcOutput};
// use std::process::Command;
use crate::{args::OutputFormat, cbmc_output_parser::get_cbmc_output};
// use std::str::FromStr;
// use tracing::debug;

use crate::session::KaniSession;

impl KaniSession {
    /// Display the results of a CBMC run in a user-friendly manner.
    pub fn  format_cbmc_output(&self, mut cbmc_process: Child) {
        // let mut args: Vec<OsString> = vec![
        //     self.cbmc_json_parser_py.clone().into(),
        //     file.into(),
        //     self.args.output_format.to_string().to_lowercase().into(),
        // ];
        let output_format_str = self.args.output_format.to_string();
        // println!("CBMC output args: {:?}", args);
        // let output_format = OutputFormat::from_str(output_format_str);
        call_loop(cbmc_process);
        // let cbmc_output= get_cbmc_output(file);
        // println!("{:?}", cbmc_output);
        // for message in cbmc_output.messages.iter() {
        //     println!("{:?}", message);
        // }
        // println!("{}", fmt_regular(cbmc_output));

        // let mut_cbmc_output = cbmc_output;
        // let kani_output = postprocess_output(cbmc_output, self.args.extra_pointer_checks);
        // for prop in cbmc_output.properties.iter() {
        //     println!("{:?}", prop);
        // }
        // let format_output = transform_cbmc_output(cbmc_output, output_format, extra_ptr_checks)?;
        // if !self.args.emit_output {
        //     print_output(format_output); // can this fail?
        // } else {
        //     emit_output(format_output)?;
        // }

        // Ok(())
    }
}
