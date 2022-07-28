// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::Result;
use pathdiff::diff_paths;
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, ChildStdout},
};
use structopt::lazy_static::lazy_static;

lazy_static! {
    static ref CBMC_DESCRIPTIONS: HashMap<&'static str, Vec<(&'static str, Option<&'static str>)>> = {
        let mut map = HashMap::new();
        map.insert("error_label", vec![]);
        map.insert("division-by-zero", vec![("division by zero", None)]);
        map.insert("enum-range-check", vec![("enum range check", None)]);
        map.insert(
            "undefined-shift",
            vec![
                ("shift distance is negative", None),
                ("shift distance too large", None),
                ("shift operand is negative", None),
                ("shift of non-integer type", None),
            ],
        );
        map.insert(
            "overflow",
            vec![
                ("result of signed mod is not representable", None),
                ("arithmetic overflow on signed type conversion", None),
                ("arithmetic overflow on signed division", None),
                ("arithmetic overflow on signed unary minus", None),
                ("arithmetic overflow on signed shl", None),
                ("arithmetic overflow on unsigned unary minus", None),
                ("arithmetic overflow on signed +", Some("arithmetic overflow on signed addition")),
                (
                    "arithmetic overflow on signed -",
                    Some("arithmetic overflow on signed subtraction"),
                ),
                (
                    "arithmetic overflow on signed *",
                    Some("arithmetic overflow on signed multiplication"),
                ),
                (
                    "arithmetic overflow on unsigned +",
                    Some("arithmetic overflow on unsigned addition"),
                ),
                (
                    "arithmetic overflow on unsigned -",
                    Some("arithmetic overflow on unsigned subtraction"),
                ),
                (
                    "arithmetic overflow on unsigned *",
                    Some("arithmetic overflow on unsigned multiplication"),
                ),
                ("arithmetic overflow on floating-point typecast", None),
                ("arithmetic overflow on floating-point division", None),
                ("arithmetic overflow on floating-point addition", None),
                ("arithmetic overflow on floating-point subtraction", None),
                ("arithmetic overflow on floating-point multiplication", None),
                ("arithmetic overflow on unsigned to signed type conversion", None),
                ("arithmetic overflow on float to signed integer type conversion", None),
                ("arithmetic overflow on signed to unsigned type conversion", None),
                ("arithmetic overflow on unsigned to unsigned type conversion", None),
                ("arithmetic overflow on float to unsigned integer type conversion", None),
            ],
        );
        map.insert(
            "NaN",
            vec![
                ("NaN on +", Some("NaN on addition")),
                ("NaN on -", Some("NaN on subtraction")),
                ("NaN on /", Some("NaN on division")),
                ("NaN on *", Some("NaN on multiplication")),
            ],
        );
        map.insert("pointer", vec![("same object violation", None)]);
        map.insert(
            "pointer_arithmetic",
            vec![
                ("pointer relation: deallocated dynamic object", None),
                ("pointer relation: dead object", None),
                ("pointer relation: pointer NULL", None),
                ("pointer relation: pointer invalid", None),
                ("pointer relation: pointer outside dynamic object bounds", None),
                ("pointer relation: pointer outside object bounds", None),
                ("pointer relation: invalid integer address", None),
                ("pointer arithmetic: deallocated dynamic object", None),
                ("pointer arithmetic: dead object", None),
                ("pointer arithmetic: pointer NULL", None),
                ("pointer arithmetic: pointer invalid", None),
                ("pointer arithmetic: pointer outside dynamic object bounds", None),
                ("pointer arithmetic: pointer outside object bounds", None),
                ("pointer arithmetic: invalid integer address", None),
            ],
        );
        map.insert(
            "pointer_dereference",
            vec![
                (
                    "dereferenced function pointer must be",
                    Some("dereference failure: invalid function pointer"),
                ),
                ("dereference failure: pointer NULL", None),
                ("dereference failure: pointer invalid", None),
                ("dereference failure: deallocated dynamic object", None),
                ("dereference failure: dead object", None),
                ("dereference failure: pointer outside dynamic object bounds", None),
                ("dereference failure: pointer outside object bounds", None),
                ("dereference failure: invalid integer address", None),
            ],
        );
        map.insert(
            "pointer_primitives",
            vec![
                ("pointer invalid", None),
                ("deallocated dynamic object", Some("pointer to deallocated dynamic object")),
                ("dead object", Some("pointer to dead object")),
                ("pointer outside dynamic object bounds", None),
                ("pointer outside object bounds", None),
                ("invalid integer address", None),
            ],
        );
        map.insert(
            "array_bounds",
            vec![
                ("lower bound", Some("index out of bounds")),
                (
                    "upper bound",
                    Some(
                        "index out of bounds: the length is less than or equal to the given index",
                    ),
                ),
            ],
        );
        map.insert(
            "bit_count",
            vec![
                ("count trailing zeros is undefined for value zero", None),
                ("count leading zeros is undefined for value zero", None),
            ],
        );
        map.insert("memory-leak", vec![("dynamically allocated memory never freed", None)]);
        map
    };
}
const UNSUPPORTED_CONSTRUCT_DESC: &str = "is not currently supported by Kani";
const UNWINDING_ASSERT_DESC: &str = "unwinding assertion loop";
const ASSERTION_FALSE: &str = "assertion false";
const DEFAULT_ASSERTION: &str = "assertion";
const REACH_CHECK_DESC: &str = "[KANI_REACHABILITY_CHECK]";

#[derive(Debug)]
pub struct CbmcOutput {
    pub messages: Vec<Message>,
    pub properties: Vec<Property>,
}

// DeepDive: This is the actual objects being parsed,
// do we want to rename to `OriginalItem` and have
// `ExtendedItem` where we extend them with more information?
// For example, verbosity level.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ParserItem {
    Program {
        program: String,
    },
    #[serde(rename_all = "camelCase")]
    Message {
        message_text: String,
        message_type: String,
    },
    Result {
        result: Vec<Property>,
    },
    #[serde(rename_all = "camelCase")]
    ProverStatus {
        _c_prover_status: String,
    },
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let check_str = match self {
            CheckStatus::Success => "SUCCESS",
            CheckStatus::Failure => "FAILURE",
            CheckStatus::Unreachable => "UNREACHABLE",
            CheckStatus::Undetermined => "UNDETERMINED",
        };
        write! {f, "{}", check_str}
    }
}

fn filepath(file: String) -> String {
    let file_path = PathBuf::from(file.clone());
    let cur_dir = env::current_dir().unwrap();

    let diff_path = diff_paths(file_path, cur_dir);
    if diff_path.is_some() {
        diff_path.unwrap().into_os_string().into_string().unwrap()
    } else {
        file
    }
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fmt_str = String::new();
        if self.file.is_some() {
            let file_str = format!("{}", filepath(self.file.clone().unwrap()));
            fmt_str.push_str(file_str.as_str());
            if self.line.is_some() {
                let line_str = format!(":{}", self.line.clone().unwrap());
                fmt_str.push_str(line_str.as_str());
                if self.column.is_some() {
                    let column_str = format!(":{}", self.column.clone().unwrap());
                    fmt_str.push_str(column_str.as_str());
                }
            }
        } else {
            fmt_str.push_str("Unknown File");
        }
        if self.function.is_some() {
            let fun_str = format!(" in function {}", self.function.clone().unwrap());
            fmt_str.push_str(fun_str.as_str());
        }

        write! {f, "{}", fmt_str}
    }
}

use crate::args::OutputFormat;
impl std::fmt::Display for ParserItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ParserItem::Program { program } => write!(f, "{}", program),
            ParserItem::Message { message_text, .. } => write!(f, "{}", message_text),
            _ => write!(f, "Not implemented!"),
        }
    }
}
#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(rename = "messageText")]
    pub txt: String,
    #[serde(rename = "messageType")]
    pub typ: String,
}

#[derive(Debug, Deserialize)]
pub struct Program {
    pub program: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Property {
    pub description: String,
    pub property: String,
    #[serde(rename = "sourceLocation")]
    pub source_location: SourceLocation,
    pub status: CheckStatus,
    pub reach: Option<CheckStatus>,
    pub trace: Option<Vec<TraceItem>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceLocation {
    pub column: Option<String>,
    pub file: Option<String>,
    pub function: Option<String>,
    pub line: Option<String>,
}

impl SourceLocation {
    fn is_missing(&self) -> bool {
        self.file.is_none() && self.function.is_none()
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceItem {
    pub thread: u32,
    pub step_type: String,
    pub hidden: bool,
    pub source_location: Option<SourceLocation>,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckStatus {
    Failure,
    Success,
    Undetermined,
    Unreachable,
}

trait Printer {
    fn print_item(item: ParserItem);
}

struct AllPrinter {}

impl Printer for AllPrinter {
    fn print_item(item: ParserItem) {
        println!("{}", item)
    }
}

struct Parser<'a, 'b> {
    pub input_so_far: String,
    pub buffer: &'a mut BufReader<&'b mut ChildStdout>,
}

#[derive(PartialEq)]
enum Action {
    ClearInput,
    ProcessItem,
}

impl<'a, 'b> Parser<'a, 'b> {
    pub fn new(buffer: &'a mut BufReader<&'b mut ChildStdout>) -> Self {
        Parser { input_so_far: String::new(), buffer: buffer }
    }

    fn triggers_action(&self, input: String) -> Option<Action> {
        if input.starts_with("[") || input.starts_with("]") {
            return Some(Action::ClearInput);
        }
        if input.starts_with("  }") {
            return Some(Action::ProcessItem);
        }
        None
    }

    fn clear_input(&mut self) {
        self.input_so_far = String::new();
    }

    fn do_action(&mut self, action: Action) -> Option<ParserItem> {
        match action {
            Action::ClearInput => {
                self.clear_input();
                None
            }
            Action::ProcessItem => {
                let item = self.parse_item();
                self.clear_input();
                Some(item)
            }
        }
    }

    fn add_to_input(&mut self, input: String) {
        self.input_so_far.push_str(input.as_str());
    }

    fn parse_item(&self) -> ParserItem {
        // println!("{}", self.counter);
        // println!("ranges: {} {}", 0, self.input_so_far.len()-limit);
        // println!("{}", &self.input_so_far.as_str()[0..self.input_so_far.len()-limit]);

        let string_without_delimiter = &self.input_so_far.as_str()[0..self.input_so_far.len() - 2];
        let block: Result<ParserItem, _> = serde_json::from_str(string_without_delimiter);
        if block.is_ok() {
            return block.unwrap();
        }
        let complete_string = &self.input_so_far.as_str()[0..self.input_so_far.len()];
        let block: Result<ParserItem, _> = serde_json::from_str(complete_string);
        assert!(block.is_ok());
        block.unwrap()
    }

    pub fn process_line(&mut self, input: String) -> Option<ParserItem> {
        self.add_to_input(input.clone());
        let action_required = self.triggers_action(input.clone());
        if action_required.is_some() {
            let action = action_required.unwrap();
            let possible_item = self.do_action(action);
            return possible_item;
        }
        None
    }
}

impl<'a, 'b> Iterator for Parser<'a, 'b> {
    type Item = ParserItem;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut input = String::new();
            match self.buffer.read_line(&mut input) {
                Ok(len) => {
                    if len == 0 {
                        return None;
                    }
                    let item = self.process_line(input);
                    if item.is_some() {
                        return item;
                    } else {
                        continue;
                    }
                }
                Err(error) => {
                    panic!("Error: Got error {} while parsing the output.", error);
                }
            }
        }
    }
}

fn process_item(item: ParserItem, extra_ptr_checks: bool, res: &mut bool) -> ParserItem {
    match item {
        ParserItem::Result { result } => {
            let (postprocessed_result, overall_status) =
                postprocess_result(result, extra_ptr_checks);
            *res = overall_status;
            ParserItem::Result { result: postprocessed_result }
        }
        ParserItem::Message { ref message_type, .. } if message_type == "ERROR" => {
            postprocess_error_message(item)
        }
        item => item,
    }
}

fn postprocess_error_message(message: ParserItem) -> ParserItem {
    if let ParserItem::Message { ref message_text, message_type: _ } = message && message_text.contains("use the `--object-bits n` option") {
        ParserItem::Message {
            message_text: message_text.replace("--object-bits ", "--enable-unstable --cbmc-args --object-bits "),
            message_type: String::from("ERROR") }
    } else {
        message
    }
}
fn must_be_skipped(item: &ParserItem) -> bool {
    matches!(item, ParserItem::Message { message_text, .. } if message_text.starts_with("Building error trace"))
        || matches!(item, ParserItem::Message { message_text, .. } if message_text.starts_with("VERIFICATION"))
}

pub fn call_loop(mut cmd: Child, extra_ptr_checks: bool, output_format: &OutputFormat) -> bool {
    let stdout = cmd.stdout.as_mut().unwrap();
    let mut stdout_reader = BufReader::new(stdout);
    let parser = Parser::new(&mut stdout_reader);
    let mut result = false;

    for item in parser {
        if must_be_skipped(&item) {
            continue;
        }
        // dbg!(&item);
        let trans_item = process_item(item, extra_ptr_checks, &mut result);
        // if add_items.is_some() {
        //     result = add_items.unwrap();
        // }
        let formatted_item = format_item(&trans_item, &output_format);
        if formatted_item.is_some() {
            println!("{}", formatted_item.unwrap())
        };
    }
    result
}

fn format_item(item: &ParserItem, output_format: &OutputFormat) -> Option<String> {
    match output_format {
        OutputFormat::Old => todo!(),
        OutputFormat::Regular => format_item_regular(item),
        OutputFormat::Terse => format_item_terse(item),
    }
}

fn format_item_regular(item: &ParserItem) -> Option<String> {
    match item {
        ParserItem::Program { program } => Some(format!("{}", program)),
        ParserItem::Message { message_text, .. } => Some(format!("{}", message_text)),
        ParserItem::Result { result } => Some(format_result(result, true)),
        _ => None,
    }
}

fn format_item_terse(item: &ParserItem) -> Option<String> {
    match item {
        ParserItem::Result { result } => Some(format_result(result, false)),
        _ => None,
    }
}

fn format_result(properties: &Vec<Property>, show_checks: bool) -> String {
    let mut result_str = String::new();
    let mut number_tests_failed = 0;
    let mut number_tests_unreachable = 0;
    let mut number_tests_undetermined = 0;
    let mut failed_tests: Vec<&Property> = vec![];

    let mut index = 1;

    if show_checks {
        result_str.push_str("\nRESULTS:\n");
    }

    for prop in properties {
        let name = &prop.property;
        let status = &prop.status;
        let description = &prop.description;
        let location = &prop.source_location;

        match status {
            CheckStatus::Failure => {
                number_tests_failed += 1;
                failed_tests.push(&prop);
            }
            CheckStatus::Undetermined => {
                number_tests_undetermined += 1;
            }
            CheckStatus::Unreachable => {
                number_tests_unreachable += 1;
                // failed checks
            }
            _ => (),
        }

        if show_checks {
            let check_id = format!("Check {}: {}\n", index, name);
            let status_msg = format!("\t - Status: {}\n", status);
            let descrition_msg = format!("\t - Description: \"{}\"\n", description);

            result_str.push_str(check_id.as_str());
            result_str.push_str(status_msg.as_str());
            result_str.push_str(descrition_msg.as_str());

            if !location.is_missing() {
                let location_msg = format!("\t - Location: {}\n", location);
                result_str.push_str(location_msg.as_str());
            }
            result_str.push_str("\n");
        }

        index += 1;
    }

    if show_checks {
        result_str.push_str("\nSUMMARY:");
    } else {
        result_str.push_str("\nVERIFICATION RESULT:");
    }
    let summary = format!("\n ** {} of {} failed", number_tests_failed, properties.len());
    result_str.push_str(summary.as_str());

    let mut other_status = Vec::<String>::new();
    if number_tests_undetermined > 0 {
        let undetermined_str = format!("{} undetermined", number_tests_undetermined);
        other_status.push(undetermined_str);
    }
    if number_tests_unreachable > 0 {
        let unreachable_str = format!("{} unreachable", number_tests_unreachable);
        other_status.push(unreachable_str);
    }
    if other_status.len() > 0 {
        result_str.push_str(" (");
        result_str.push_str(&other_status.join(","));
        result_str.push_str(")");
    }
    result_str.push_str("\n");

    for prop in failed_tests {
        let failure_message = build_failure_message(prop.description.clone(), &prop.trace.clone());
        result_str.push_str(failure_message.as_str());
    }

    let verification_result = if number_tests_failed == 0 { "SUCCESSFUL " } else { "FAILED" };
    let overall_result = format!("\nVERIFICATION:- {}\n", verification_result);
    result_str.push_str(overall_result.as_str());

    if has_check_failures(&properties, UNSUPPORTED_CONSTRUCT_DESC) {
        result_str.push_str(
            "** WARNING: A Rust construct that is not currently supported \
        by Kani was found to be reachable. Check the results for \
        more details.",
        );
    }
    if has_check_failures(&properties, UNWINDING_ASSERT_DESC) {
        result_str.push_str("[Kani] info: Verification output shows one or more unwinding failures.\n\
        [Kani] tip: Consider increasing the unwinding value or disabling `--unwinding-assertions`.\n");
    }

    result_str
}

fn build_failure_message(description: String, trace: &Option<Vec<TraceItem>>) -> String {
    let backup_failure_message = format!("Failed Checks: {}\n", description);
    if trace.is_none() {
        return backup_failure_message;
    }
    let failure_trace = trace.clone().unwrap();

    let failure_source_wrap = failure_trace[failure_trace.len() - 1].source_location.clone();
    if failure_source_wrap.is_none() {
        return backup_failure_message;
    }
    let failure_source = failure_source_wrap.unwrap();

    if failure_source.file.is_some()
        && failure_source.function.is_some()
        && failure_source.line.is_some()
    {
        let failure_file = failure_source.file.unwrap();
        let failure_function = failure_source.function.unwrap();
        let failure_line = failure_source.line.unwrap();
        return format!(
            "Failed Checks: {}\n File: \"{}\", line {}, in {}\n",
            description, failure_file, failure_line, failure_function
        );
    }
    backup_failure_message
}

pub fn postprocess_result(
    properties: Vec<Property>,
    extra_ptr_checks: bool,
) -> (Vec<Property>, bool) {
    let has_reachable_unsupported_constructs =
        has_check_failures(&properties, UNSUPPORTED_CONSTRUCT_DESC);
    let has_failed_unwinding_asserts = has_check_failures(&properties, UNWINDING_ASSERT_DESC);
    // println!("properties: {:?}\n", properties);
    let (properties_with_undefined, has_reachable_undefined_functions) =
        modify_undefined_function_checks(properties);
    // println!("properties_with_undefined: {:?}\n", properties_with_undefined);
    let (properties_without_reachs, reach_checks) = filter_reach_checks(properties_with_undefined);
    // println!("properties_without_reachs: {:?}\n", properties_without_reachs);
    // println!("reach_checks: {:?}\n", reach_checks);
    let properties_without_sanity_checks = filter_sanity_checks(properties_without_reachs);
    // println!("properties_without_sanity_checks: {:?}\n", properties_without_sanity_checks);
    let properties_annotated =
        annotate_properties_with_reach_results(properties_without_sanity_checks, reach_checks);
    // println!("properties_annotated: {:?}\n", properties_annotated);
    let properties_without_ids = remove_check_ids_from_description(properties_annotated);
    // println!("properties_without_ids: {:?}\n", properties_without_ids);

    let new_properties = if !extra_ptr_checks {
        filter_ptr_checks(properties_without_ids)
    } else {
        properties_without_ids
    };
    let has_fundamental_failures = has_reachable_unsupported_constructs
        || has_failed_unwinding_asserts
        || has_reachable_undefined_functions;
    let final_properties = final_changes(new_properties, has_fundamental_failures);
    // TODO: Return a flag or messages?
    let overall_result = determine_result(&final_properties);
    (final_properties, overall_result)
}

fn determine_result(properties: &Vec<Property>) -> bool {
    let number_failed =
        properties.iter().filter(|prop| prop.status == CheckStatus::Failure).count();
    number_failed == 0
}

fn get_readable_description(property: &Property) -> String {
    let original = property.description.clone();
    let class_id = extract_property_class(property).unwrap();
    // dbg!(&class_id);
    let description_alternatives = CBMC_DESCRIPTIONS.get(class_id);
    if description_alternatives.is_some() {
        let alt_descriptions = description_alternatives.unwrap();
        for (desc_to_match, opt_desc_to_replace) in alt_descriptions {
            if original.contains(desc_to_match) {
                if opt_desc_to_replace.is_some() {
                    let desc_to_replace = opt_desc_to_replace.unwrap();
                    return desc_to_replace.to_string();
                } else {
                    return desc_to_match.to_string();
                }
            }
        }
    }
    return original;
}

fn final_changes(mut properties: Vec<Property>, has_fundamental_failures: bool) -> Vec<Property> {
    for prop in properties.iter_mut() {
        prop.description = get_readable_description(&prop);
        if has_fundamental_failures {
            if prop.status == CheckStatus::Success {
                prop.status = CheckStatus::Undetermined;
            }
        } else if prop.reach.is_some() && prop.reach.unwrap() == CheckStatus::Success {
            let description = &prop.description;
            assert!(
                prop.status == CheckStatus::Success,
                "** ERROR: Expecting the unreachable property \"{}\" to have a status of \"SUCCESS\"",
                description
            );
            prop.status = CheckStatus::Unreachable
        }
    }
    properties
}

fn filter_ptr_checks(properties: Vec<Property>) -> Vec<Property> {
    let props = properties
        .into_iter()
        .filter(|prop| {
            !extract_property_class(prop).unwrap().contains("pointer_arithmetic")
                && !extract_property_class(prop).unwrap().contains("pointer_primitives")
        })
        .collect();
    props
}
fn remove_check_ids_from_description(mut properties: Vec<Property>) -> Vec<Property> {
    let re = Regex::new(r"\[KANI_CHECK_ID_.*_([0-9])*\] ").unwrap();
    for prop in properties.iter_mut() {
        prop.description = re.replace(prop.description.as_str(), "").to_string();
    }
    properties
}

fn modify_undefined_function_checks(mut properties: Vec<Property>) -> (Vec<Property>, bool) {
    let mut has_unknown_location_checks = false;
    for mut prop in &mut properties {
        if prop.description.contains(ASSERTION_FALSE)
            && extract_property_class(&prop).unwrap() == DEFAULT_ASSERTION
            && prop.source_location.file.is_none()
        {
            prop.description = "Function with missing definition is unreachable".to_string();
            if prop.status == CheckStatus::Failure {
                has_unknown_location_checks = true;
            }
        };
    }
    (properties, has_unknown_location_checks)
}

fn extract_property_class(property: &Property) -> Option<&str> {
    let property_class: Vec<&str> = property.property.rsplitn(3, ".").collect();
    if property_class.len() > 1 { Some(property_class[1]) } else { None }
}

fn filter_reach_checks(properties: Vec<Property>) -> (Vec<Property>, Vec<Property>) {
    filter_properties(properties, REACH_CHECK_DESC)
}

fn filter_properties(properties: Vec<Property>, message: &str) -> (Vec<Property>, Vec<Property>) {
    let mut filtered_properties = Vec::<Property>::new();
    let mut removed_properties = Vec::<Property>::new();
    for prop in properties {
        if prop.description.contains(message) {
            removed_properties.push(prop);
        } else {
            filtered_properties.push(prop);
        }
    }
    (filtered_properties, removed_properties)
}

fn filter_sanity_checks(properties: Vec<Property>) -> Vec<Property> {
    properties
        .into_iter()
        .filter(|prop| {
            !(extract_property_class(prop).unwrap() == "sanity_check"
                && prop.status == CheckStatus::Success)
        })
        .collect()
}

fn annotate_properties_with_reach_results(
    mut properties: Vec<Property>,
    reach_checks: Vec<Property>,
) -> Vec<Property> {
    let re = Regex::new("KANI_CHECK_ID_.*_([0-9])*").unwrap();
    let mut hash_map: HashMap<String, CheckStatus> = HashMap::new();
    for reach_check in reach_checks {
        let description = reach_check.description;
        let check_id = re.captures(description.as_str()).unwrap().get(0).unwrap().as_str();
        let check_id_str = format!("[{}]", check_id);
        let status = reach_check.status;
        let res_ins = hash_map.insert(check_id_str, status);
        assert!(res_ins.is_none());
    }
    // dbg!(&hash_map);
    for prop in properties.iter_mut() {
        let description = &prop.description;
        // let id_str = format!("\\[{}\\]", description);
        // let match_obj = re.captures(id_str.as_str());
        let re2 = Regex::new(r"\[KANI_CHECK_ID_([^\]]*)\]").unwrap();
        if re2.is_match(description) {
            let prop_match_id =
                re2.captures(description.as_str()).unwrap().get(0).unwrap().as_str();
            // dbg!(&prop_match_id);
            let status_from = hash_map.get(&prop_match_id.to_string());
            // assert!(status_from.is_some());
            if status_from.is_some() {
                prop.reach = Some(*status_from.unwrap());
            }
        }
    }
    properties
}

fn has_check_failures(properties: &Vec<Property>, message: &str) -> bool {
    let properties_with = properties
        .iter()
        .filter(|prop| prop.description.contains(message) && prop.status == CheckStatus::Failure)
        .count();
    return properties_with > 0;
}
