#[macro_use]
mod ansi;
mod analyser;
mod ast;
mod codegen;
mod diagnostics;
mod loader;
mod optimiser;
mod parser;
mod tokeniser;
mod util;

use analyser::{Analyser, AnalyserResult};
pub use codegen::Codegen;
use diagnostics::{Diagnostic, Diagnostics};
use loader::Loader;
pub use optimiser::Optimiser;

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
    let (diagnostics, main, circs, mut loader) = match analyser.analyse() {
        AnalyserResult::Success {
            diagnostics,
            main,
            circs,
            loader,
        } => (diagnostics, main, circs, loader),
        AnalyserResult::Error {
            diagnostics,
            mut loader,
        } => {
            for diagnostic in diagnostics {
                eprintln!(
                    "{}",
                    diagnostic
                        .to_string(&mut loader)
                        .unwrap_or_else(|_| unreachable!("format failure"))
                );
            }
            return ExitCode::FAILURE;
        }
    };

    for diagnostic in diagnostics {
        eprintln!(
            "{}",
            diagnostic
                .to_string(&mut loader)
                .unwrap_or_else(|_| unreachable!("format failure"))
        );
    }

    let optimiser_units = Optimiser::optimise_circs(circs, main);

    let ir_blocks = Codegen::emit_ir(&optimiser_units, main);
    let optimised_blocks = Optimiser::optimise_ir(ir_blocks);
    println!("{optimised_blocks}");

    codegen::targets::x86_64_linux_gas::generate(optimised_blocks, "main");

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
