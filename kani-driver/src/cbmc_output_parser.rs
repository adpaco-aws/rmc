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
pub struct CBMCResult {
    // #[serde(rename="result")]
    pub result: Vec<Property>,
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

#[derive(Copy, Clone)]
enum ParserState {
    Initial,
    Waiting,
    Processing,
    Processed,
    Final,
}

struct Parser {
    pub state: ParserState,
    pub input_so_far: String,
    pub counter: u32,
}


#[derive(Debug, Deserialize)]
enum ParserItem {
    Program,
    Message,
}

impl Parser {
    pub fn new() -> Self {
        Parser { state: ParserState::Initial, input_so_far: String::new() , counter: 0 }
    }

    fn is_acceptable(&self, input: String) -> bool {
        match self.state {
            ParserState::Initial => input.starts_with("["),
            ParserState::Waiting => input.starts_with("  {"),
            ParserState::Processing => true,
            ParserState::Processed => input.starts_with("  }"),
            ParserState::Final => input.len() == 0,
            // Initial => input.starts_with("["),
        }
    }

    fn add_to_input(&mut self, input: String) {
        self.counter += 1;
        self.input_so_far.push_str(input.as_str());
    }

    fn parsed_complete_item(&self) -> bool {
        let limit = if self.input_so_far.len() > 2 { 2 } else {0};
        println!("{}", self.counter);
        println!("ranges: {} {}", 0, self.input_so_far.len()-limit);
        println!("{}", &self.input_so_far.as_str()[0..self.input_so_far.len()-limit]);
        // let block: Result<ParserItem, _> = serde_json::from_str(&self.input_so_far.as_str()[0..self.input_so_far.len()-limit]);
        // if block.is_ok() {
        //     println!("{:?}", &block);
        //     return true;
        // }
        // let block: Result<Message, _> = serde_json::from_str(&self.input_so_far.as_str()[0..self.input_so_far.len()-limit]);
        // // let result: Result<, _> = serde_json::from_str(&self.input_so_far.as_str()[0..self.input_so_far.len()-limit]);
        // if block.is_ok() {
        //     println!("{:?}", &block);
        //     return true;
        // }
        // let block: Result<CBMCResult, _> = serde_json::from_str(&self.input_so_far.as_str()[0..self.input_so_far.len()-limit]);
        // if block.is_ok() {
        //     println!("{:?}", &block);
        //     return true;
        // }
        false
    }

    fn check_transition(&mut self) {
        let has_transition = 
        match self.state {
            ParserState::Initial => {self.state = ParserState::Waiting; true }
            ParserState::Waiting => { self.state = ParserState::Processing; false }
            // ParserState::Processing => { false }
            ParserState::Processing => { let flag = self.parsed_complete_item(); if flag { self.state = ParserState::Waiting}; flag }
            _ => false
        };
        //     _ => false
        // };
        if has_transition {
            self.input_so_far = String::new()
        }
        // println!("hey");
        // if has_transition {
        //     self.input_so_far = String::new()
        // }
    }

    pub fn process_line(&mut self, input: String) {
        if !self.is_acceptable(input.clone()) {
            panic!("unexpected input");
        } else {
            self.add_to_input(input)
        }
        self.check_transition();
    }

}

pub fn call_loop(mut cmd: Child) {
    // let stdout = cmd.stdout.as_mut().unwrap();
    let stdout = cmd.stdout.as_mut().unwrap();
    let mut stdout_reader = BufReader::new(stdout);
    let mut parser = Parser::new();
    // let stream = Deserializer::from_reader(&mut stdout_reader).into_iter::<Value>();
    // let pars = JsonStreamstdout_reader
    // let mut counter = 0;
    // for value in stream {
    //     println!("{}", value.unwrap());
    //     counter += 1;
    // }

    // println!("{}", counter);
    loop {
        let mut input = String::new();
        match stdout_reader.read_line(&mut input) {
            Ok(len) => if len == 0 {
                println!("{}", parser.input_so_far);
                return;
            } else {
                parser.process_line(input);
            } 
            Err(error) => {
                eprintln!("error: {}", error);
                return;
            }
        }
    }

}
fn extract_error_messages(_messages: &Vec<Message>) -> Vec<Message> {
    Vec::<Message>::new()
}

pub fn get_cbmc_output(path: &Path) -> CbmcOutput {
    let file = fs::File::open(path).unwrap();
    let reader = BufReader::new(file);
    let json_array: Vec<Value> = serde_json::from_reader(reader).unwrap();
    let (all_messages, properties) = extract_messages_and_properties(&json_array);
    let error_messages = extract_error_messages(&all_messages);
    if error_messages.len() > 0 {
        panic!("errors!!");
    }
    let messages = filter_messages(all_messages);
    CbmcOutput { messages, properties }
}



pub fn postprocess_output(mut cbmc_output: CbmcOutput, extra_checks: bool) -> CbmcOutput {
    // has_reachable_unsupported_constructs = has_check_failure(properties, GlobalMessages.UNSUPPORTED_CONSTRUCT_DESC)
    // has_failed_unwinding_asserts = has_check_failure(properties, GlobalMessages.UNWINDING_ASSERT_DESC)
    // has_reachable_undefined_functions = modify_undefined_function_checks(properties)
    // properties, reach_checks = filter_reach_checks(properties)
    // properties = filter_sanity_checks(properties)
    // annotate_properties_with_reach_results(properties, reach_checks)
    // remove_check_ids_from_description(properties)

    // if not extra_ptr_check:
    //     properties = filter_ptr_checks(properties)
    let has_reachable_unsupported_constructs = has_check_failures(&cbmc_output.properties, UNSUPPORTED_CONSTRUCT_DESC);
    let has_failed_unwinding_asserts = has_check_failures(&cbmc_output.properties, UNWINDING_ASSERT_DESC);
    // let has_reachable_undefined_functions = modify_undefined_function_checks()


    cbmc_output
}

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