use std::error::Error;
use std::fs;
use std::io;
use std::io::Read;
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};
use shared::error::AppError;

use crate::frame_dump::DirectionKind;
use crate::frame_encoding::FrameEncodingKind;

mod frame_dump;
mod frame_encoding;

// minimer prefixes every AppError message with this.
const APP_ERROR_MESSAGE_PREFIX: &str = "Error: ";

fn main() -> ExitCode {
    env_logger::init();

    let matches: ArgMatches = create_command().get_matches();
    let dump_result: Result<String, AppError> = run_subcommand(&matches);

    match dump_result {
        Ok(dump) => {
            print!("{dump}");

            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", get_error_text(&error));

            ExitCode::FAILURE
        }
    }
}

fn create_command() -> Command {
    Command::new("protocol_dump")
        .about("Decodes captured frames and replay files")
        .subcommand_required(true)
        .subcommand(
            Command::new("frame")
                .about("Decodes one frame and prints its Debug form")
                .arg(Arg::new("direction").long("direction").required(true).value_parser(frame_dump::DIRECTION_NAMES))
                .arg(
                    Arg::new("hex")
                        .long("hex")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("base64")
                        .help("The frame is written as hexadecimal digits"),
                )
                .arg(
                    Arg::new("base64")
                        .long("base64")
                        .action(ArgAction::SetTrue)
                        .help("The frame is written as base64, as copied from browser devtools"),
                )
                .arg(Arg::new("file").value_name("FILE").help("Read from standard input when absent")),
        )
}

fn run_subcommand(matches: &ArgMatches) -> Result<String, AppError> {
    match matches.subcommand() {
        Some(("frame", frame_matches)) => run_frame(frame_matches),
        Some((other, _)) => Err(AppError::new(&format!("unknown subcommand {other}"))),
        None => Err(AppError::new("missing subcommand")),
    }
}

fn run_frame(frame_matches: &ArgMatches) -> Result<String, AppError> {
    let direction_name: &String = frame_matches.get_one::<String>("direction").expect("direction is required via clap");
    let direction: DirectionKind = DirectionKind::try_from(direction_name.as_str())?;
    let encoding: FrameEncodingKind = get_frame_encoding(frame_matches);
    let input_bytes: Vec<u8> = read_input(frame_matches.get_one::<String>("file"))?;
    let frame_bytes: Vec<u8> = frame_encoding::decode_frame_input(&input_bytes, encoding)?;

    frame_dump::dump_frame(&frame_bytes, direction)
}

fn get_frame_encoding(frame_matches: &ArgMatches) -> FrameEncodingKind {
    if frame_matches.get_flag("hex") {
        return FrameEncodingKind::Hex;
    }

    if frame_matches.get_flag("base64") {
        return FrameEncodingKind::Base64;
    }

    FrameEncodingKind::Binary
}

fn read_input(file_path: Option<&String>) -> Result<Vec<u8>, AppError> {
    let Some(file_path): Option<&String> = file_path else {
        return read_standard_input();
    };

    fs::read(file_path).map_err(|error| AppError::from_error(&format!("cannot read {file_path}"), Box::new(error)))
}

fn read_standard_input() -> Result<Vec<u8>, AppError> {
    let mut input_bytes: Vec<u8> = Vec::new();

    io::stdin()
        .read_to_end(&mut input_bytes)
        .map_err(|error| AppError::from_error("cannot read standard input", Box::new(error)))?;

    Ok(input_bytes)
}

/// The message and its chain of causes, without backtraces.
fn get_error_text(error: &AppError) -> String {
    let Some(sub_error): Option<&Box<dyn Error>> = error.sub_error.as_ref() else {
        return error.message.clone();
    };

    let sub_error_text: String = match sub_error.downcast_ref::<AppError>() {
        Some(sub_app_error) => get_error_text(sub_app_error),
        None => sub_error.to_string(),
    };

    // A foreign error converted by `?` already carries the sub-error text as its message.
    let message_without_prefix: &str = error.message.strip_prefix(APP_ERROR_MESSAGE_PREFIX).unwrap_or(&error.message);

    if message_without_prefix == sub_error_text {
        return error.message.clone();
    }

    format!("{}: {sub_error_text}", error.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_error_text_without_sub_error_is_the_message() {
        let error: AppError = AppError::new("missing subcommand");

        assert_eq!(get_error_text(&error), "Error: missing subcommand");
    }

    #[test]
    fn get_error_text_follows_an_app_error_sub_error() {
        let inner_error: AppError = AppError::from_error("cannot decode", Box::new(io::Error::other("truncated")));
        let error: AppError = AppError::from_error("cannot dump frame", Box::new(inner_error));

        assert_eq!(
            get_error_text(&error),
            "Error: cannot dump frame: Error: cannot decode: truncated"
        );
    }

    #[test]
    fn get_error_text_appends_a_foreign_sub_error() {
        let error: AppError = AppError::from_error(
            "cannot read capture.bin",
            Box::new(io::Error::other("permission denied")),
        );

        assert_eq!(
            get_error_text(&error),
            "Error: cannot read capture.bin: permission denied"
        );
    }

    #[test]
    fn get_error_text_omits_a_foreign_sub_error_the_message_already_carries() {
        let error: AppError = AppError::from(io::Error::other("permission denied"));

        assert_eq!(get_error_text(&error), "Error: permission denied");
    }
}
