// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{path::Path, io::{BufReader, self, BufRead}, fs, process::{Child, ChildStdout}};
use serde::{Deserialize};
use anyhow::Result;
use serde_json::{Deserializer, Value};
use std::str::FromStr;
use regex::{Captures, Regex};

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
    Program { program: String },
    #[serde(rename_all = "camelCase")]
    Message {
        message_text: String,
        message_type: String,
    },
    Result { result: Vec<Property>},
    #[serde(rename_all = "camelCase")]
    ProverStatus { c_prover_status: String },
}

use std::fmt::Display;
impl std::fmt::Display for ParserItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ParserItem::Program { program } =>
                write!(f, "{}", program),
            ParserItem::Message { message_text, .. } =>
                write!(f, "{}", message_text),
            _ =>
                write!(f, "Not implemented!"),
        }
    }
}
#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(rename="messageText")]
    pub txt: String,
    #[serde(rename="messageType")]
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
    #[serde(rename="sourceLocation")]
    pub source_location: SourceLocation,
    pub status: CheckStatus,
    pub reach: Option<CheckStatus>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SourceLocation {
    pub column: Option<String>,
    pub file: Option<String>,
    pub function: String,
    pub line: Option<String>,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckStatus {
    Failure,
    Success,
    Undetermined,
    Unreachable,
}

// impl CbmcOutput {
//     pub fn new() -> Self {
//         CbmcOutput { messages: vec![] , properties: vec![] }
//     }
// }

// Placehold to filter out messages
fn filter_messages(messages: Vec<Message>) -> Vec<Message> {
    messages
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
            Action::ClearInput => { self.clear_input(); None},
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
                if len == 0 { return None; }
                let item = self.process_line(input);
                if item.is_some() {
                    return item;
                } else {
                    continue
                }
            }
            Err(error) => {
                panic!("Error: Got error {} while parsing the output.", error);
            }
        }
    }
}

}

fn process_item(item: ParserItem, extra_ptr_checks: bool) -> ParserItem {
    match item {
        ParserItem::Result { result } => {
            let postprocessed_result = postprocess_result(result, extra_ptr_checks);
            ParserItem::Result { result: postprocessed_result }
        },
        item => item,
    }
} 

pub fn call_loop(mut cmd: Child, extra_ptr_checks: bool) {
    let stdout = cmd.stdout.as_mut().unwrap();
    let mut stdout_reader = BufReader::new(stdout);
    let parser = Parser::new(&mut stdout_reader);
    // let mut printer = AllPrinter {};

    for item in parser {
        let trans_item = process_item(item, extra_ptr_checks);
        println!("{:?}", trans_item);
    }

}

pub fn postprocess_result(mut properties: Vec<Property>, extra_ptr_checks: bool) -> Vec<Property> {
    let has_reachable_unsupported_constructs = has_check_failures(&properties, UNSUPPORTED_CONSTRUCT_DESC);
    let has_failed_unwinding_asserts = has_check_failures(&properties, UNWINDING_ASSERT_DESC);
    // let new_properties: &mut Vec<Property> = properties.as_mut();
    let (properties_with_undefined, has_reachable_undefined_functions) = modify_undefined_function_checks(properties);
    let (properties_without_reachs, reach_checks) = filter_reach_checks(properties_with_undefined);
    let properties_without_sanity_checks = filter_sanity_checks(properties_without_reachs);
    annotate_properties_with_reach_results(properties_without_sanity_checks, reach_checks);
    // remove_check_ids_from_description(properties_without_sanity_checks);

    if !extra_ptr_checks {
        filter_ptr_checks(properties_without_sanity_checks);
    }
    let new_vec = Vec::<Property>::new();
    new_vec
}

fn filter_ptr_checks(mut properties: &Vec<Property>) {

}
// fn remove_check_ids_from_description(mut properties: Vec<Property>) {
// }

fn modify_undefined_function_checks(mut properties: Vec<Property>) -> (Vec<Property>, bool) {
    let mut has_unknown_location_checks = false;
    for mut prop in properties.iter_mut() {
        if prop.description.contains(ASSERTION_FALSE) &&
           extract_property_class(&prop).unwrap() == DEFAULT_ASSERTION &&
           prop.source_location.file.is_none() {
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
    if property_class.len() > 1 {
        Some(property_class[2])
    } else {
        None
    }
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
    properties.into_iter().filter(|prop| extract_property_class(prop).unwrap() == "sanity_check" && prop.status == CheckStatus::Success).collect()
}

fn annotate_properties_with_reach_results(mut properties: Vec<Property>, reach_checks: Vec<Property>) {
    let re = Regex::new("KANI_CHECK_ID_.*_([0-9])*").unwrap();
    for reach_check in reach_checks {
        let description = reach_check.description;
        let check_id = re.captures(description.as_str()).unwrap().get(0).unwrap().as_str();
        modify_reach_status(properties.to_owned(), check_id, reach_check.status);
    }
}

fn modify_reach_status(mut properties: Vec<Property>, check_id: &str, status: CheckStatus) {
    let re = Regex::new("_.*_([0-9])*").unwrap();
    for mut prop in properties.iter_mut() {
        let description = prop.description.clone();
        let id_str = format!("\\[{}\\]", description);
        let match_obj = re.captures(id_str.as_str());
        if match_obj.is_some() {
            let match_id = match_obj.unwrap().get(0).unwrap().as_str();
            let check_id_fmt = format!("[{}]", check_id);
            if match_id == check_id_fmt {
                prop.reach = Some(status);
            }
        }
    }
}
    // 
    // has_failed_unwinding_asserts = has_check_failure(properties, GlobalMessages.UNWINDING_ASSERT_DESC)
    // has_reachable_undefined_functions = modify_undefined_function_checks(properties)
    // properties, reach_checks = filter_reach_checks(properties)
    // properties = filter_sanity_checks(properties)
    // // annotate_properties_with_reach_results(properties, reach_checks)
    // // remove_check_ids_from_description(properties)

    // // if not extra_ptr_check:
    // //     properties = filter_ptr_checks(properties)
    // let has_reachable_unsupported_constructs = has_check_failures(&cbmc_output.properties, UNSUPPORTED_CONSTRUCT_DESC);
    // let has_failed_unwinding_asserts = has_check_failures(&cbmc_output.properties, UNWINDING_ASSERT_DESC);
    // // let has_reachable_undefined_functions = modify_undefined_function_checks()


    // cbmc_output
// }

// pub fn get_cbmc_output(path: &Path) -> CbmcOutput {
//     let file = fs::File::open(path).unwrap();
//     let reader = BufReader::new(file);
//     let json_array: Vec<Value> = serde_json::from_reader(reader).unwrap();
//     let (all_messages, properties) = extract_messages_and_properties(&json_array);
//     let error_messages = extract_error_messages(&all_messages);
//     if error_messages.len() > 0 {
//         panic!("errors!!");
//     }
//     let messages = filter_messages(all_messages);
//     CbmcOutput { messages, properties }
// }


fn has_check_failures(properties: &Vec<Property>, message: &str) -> bool {
    let properties_with = properties.iter().filter(|prop| prop.description.contains(message)).count();
    return properties_with > 0;
}
