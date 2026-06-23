use std::{env, error::Error};

mod btt_dialogue;
mod exd_schema;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    btt_dialogue::export(btt_dialogue::Options::parse(&args)?)
}
