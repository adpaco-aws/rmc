// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{path::Path, io::{BufReader, self, BufRead}, fs, process::Child};
use serde::{Deserialize};
use anyhow::Result;
use serde_json::{Deserializer, Value};
use std::str::FromStr;

const UNSUPPORTED_CONSTRUCT_DESC: &str = "is not currently supported by Kani";
const UNWINDING_ASSERT_DESC: &str = "unwinding assertion loop";

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

#[derive(Debug, Deserialize)]
pub struct Property {
    pub description: String,
    pub property: String,
    #[serde(rename="sourceLocation")]
    pub source_location: SourceLocation,
    pub status: CheckStatus,
}

#[derive(Debug, Deserialize)]
pub struct SourceLocation {
    pub column: Option<String>,
    pub file: Option<String>,
    pub function: String,
    pub line: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckStatus {
    Failure,
    Success,
    Undetermined,
    Unreachable,
}

impl CbmcOutput {
    pub fn new() -> Self {
        CbmcOutput { messages: vec![] , properties: vec![] }
    }
}


pub fn fmt_regular(output: CbmcOutput) -> String {
    let mut str_regular = String::new();
    output.messages.iter().map(|x| x.txt.clone()).collect::<Vec<String>>().join("\n")
}

fn extract_messages_and_properties(json_array: &Vec<Value>) -> (Vec<Message>, Vec<Property>) {
    let mut messages = Vec::<Message>::new();
    let mut properties = Vec::<Property>::new();
    for obj in json_array.iter() {
        assert!(obj.is_object());
        if obj.get("messageText").is_some() {
            let txt = obj.get("messageText").unwrap().as_str().unwrap().to_string();
            let typ = obj.get("messageType").unwrap().as_str().unwrap().to_string();
          messages.push(Message { txt, typ });
        }
    //     if let Some(message) = obj.get("program") {
    //         let message_str = message.to_string();
    //      messages.push(message_str);
    //    }
        if let Some(result) = obj.get("result") {
         let result_array = result.as_array().unwrap();
         for prop in result_array.iter() {
             let x = prop.clone();
             // println!("{:?}", prop);
             let property: Property = serde_json::from_value(x).unwrap();
             // if property.source_location.is_none() {
             //     println!("{:?}", property);
             // }
            properties.push(property);
 
             // println!("{:?}", result);
         }
    }
}
    (messages, properties)
}

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
        // println!("{:?}", item);
        println!("{}", item)
    }
}

#[derive(Copy, Clone)]
enum ParserState {
    Initial,
    Waiting,
    Processing,
    Processed,
    Final,
}

struct Parser {
    pub input_so_far: String,
}

#[derive(PartialEq)]
enum Action {
    ClearInput,
    ProcessItem,
}

impl Parser {
    pub fn new() -> Self {
        Parser { input_so_far: String::new() }
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

    fn do_action(&mut self, action: Action) {
        // DeepDive: Do we want printer to be part of parser?
        // DeepDive: Do we want logger to be part of parser?
        let printer = AllPrinter {};
        match action {
            Action::ClearInput => self.clear_input(),
            Action::ProcessItem => {
                let item = self.process_item();
                // TODO: Transform item?
                AllPrinter::print_item(item);
                // TODO: Log item?
                self.clear_input()
            }
            // Another action? Produce JSON file
        }
    }

    fn add_to_input(&mut self, input: String) {
        self.input_so_far.push_str(input.as_str());
    }

    fn process_item(&self) -> ParserItem {
        // println!("{}", self.counter);
        // println!("ranges: {} {}", 0, self.input_so_far.len()-limit);
        // println!("{}", &self.input_so_far.as_str()[0..self.input_so_far.len()-limit]);

        let block: Result<ParserItem, _> = serde_json::from_str(&self.input_so_far.as_str()[0..self.input_so_far.len()-2]);
        if block.is_ok() {
            return block.unwrap();
        }

        let block: Result<ParserItem, _> = serde_json::from_str(&self.input_so_far.as_str()[0..self.input_so_far.len()]);
        assert!(block.is_ok());
        return block.unwrap();
    }

    pub fn process_line(&mut self, input: String, printer: &AllPrinter) {
        let action_required = self.triggers_action(input.clone());
        self.add_to_input(input.clone());
        if action_required.is_some() {
            let action = action_required.unwrap();
            self.do_action(action);
        }
    }
}
// impl Iterator for Parser {
//     fn next(&mut self) -> Option<Self::Item> {

//     }
// }

pub fn call_loop(mut cmd: Child) {
    let stdout = cmd.stdout.as_mut().unwrap();
    let mut stdout_reader = BufReader::new(stdout);
    let mut parser = Parser::new();
    let mut printer = AllPrinter {};
    loop {
        let mut input = String::new();
        match stdout_reader.read_line(&mut input) {
            Ok(len) => if len == 0 {
                println!("{}", parser.input_so_far);
                return;
            } else {
                parser.process_line(input, &printer);
            } 
            Err(error) => {
                eprintln!("error: {}", error);
                return;
            }
        }
    }

}
// fn extract_error_messages(_messages: &Vec<Message>) -> Vec<Message> {
//     Vec::<Message>::new()
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



// pub fn postprocess_output(mut cbmc_output: CbmcOutput, extra_checks: bool) -> CbmcOutput {
    // has_reachable_unsupported_constructs = has_check_failure(properties, GlobalMessages.UNSUPPORTED_CONSTRUCT_DESC)
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

fn has_check_failures(properties: &Vec<Property>, message: &str) -> bool {
    for prop in properties.iter() {
        if prop.description.contains(&message) && prop.status == CheckStatus::Failure {
            return true
        }
    }
    false
}

// fn modify_undefined_function_checks(cbmc_output: CbmcOutput) -> bool {
//     cbmc_output.properties = vec![];
//     false
// }