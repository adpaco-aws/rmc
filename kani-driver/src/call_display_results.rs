// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::cbmc_output_parser::call_loop;
use crate::session::KaniSession;
use std::process::Child;

impl KaniSession {
    /// Display the results of a CBMC run in a user-friendly manner.
    pub fn format_cbmc_output(&self, cbmc_process: Child) -> bool {
        // let mut args: Vec<OsString> = vec![
        //     self.cbmc_json_parser_py.clone().into(),
        //     file.into(),
        //     self.args.output_format.to_string().to_lowercase().into(),
        // ];
        // println!("CBMC output args: {:?}", args);
        // let output_format = OutputFormat::from_str(output_format_str);
        call_loop(cbmc_process, self.args.extra_pointer_checks, &self.args.output_format)
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
