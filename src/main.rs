use libcircuit::{Analyser, Loader};

use std::collections::HashMap;
use std::env::{args, Args};
use std::process::ExitCode;
use std::rc::Rc;

struct ParsedArgs {
    arg0: String,
    filename: Rc<str>,
}

fn main() -> ExitCode {
    let args = match parse_args(args()) {
        Some(args) => args,
        None => return ExitCode::FAILURE,
    };

    let mut loader = Loader::new();
    match loader.load_root_module(&args.filename) {
        Ok(module) => module,
        Err(_) => {
            eprintln!("failed to load root module");
            return ExitCode::FAILURE;
        }
    };

    let analyser = Analyser::new(HashMap::new(), loader);
    match analyser.analyse() {
        Ok(result) => {
            println!("{result:#?}");
        }
        Err(ds) => {
            // for diagnostic in ds {
            //     eprintln!(
            //         "{}",
            //         diagnostic
            //             .to_string(&mut loader)
            //             .unwrap_or_else(|_| unreachable!("format failure"))
            //     );
            // }
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

fn parse_args(mut args: Args) -> Option<ParsedArgs> {
    let arg0 = match args.next() {
        Some(arg) => arg,
        None => panic!("argv[0] not set"),
    };

    let filename = match args.next() {
        Some(arg) => arg,
        None => {
            usage(&arg0);
            return None;
        }
    };

    Some(ParsedArgs {
        arg0,
        filename: Rc::from(filename),
    })
}

fn usage(arg0: &str) {
    eprintln!("usage: {arg0} <filename>")
}
