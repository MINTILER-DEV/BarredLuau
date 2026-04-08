use std::fs;
use std::path::PathBuf;

use barred_luau::config::{BuildMode, CompileConfig};
use clap::Parser as ClapParser;

#[derive(ClapParser, Debug)]
#[command(
    author,
    version,
    about = "Roblox Luau virtualization-oriented obfuscator"
)]
struct Cli {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = false)]
    debug: bool,
    #[arg(long, default_value_t = false)]
    release: bool,
    #[arg(long, default_value_t = false)]
    anti_tamper: bool,
    #[arg(long, default_value_t = false)]
    randomize_opcodes: bool,
    #[arg(long, default_value_t = 0x0BAD_5EED)]
    seed: u32,
    #[arg(long, default_value_t = 2)]
    encoder_rounds: usize,
    #[arg(long, default_value = "roblox-luau")]
    target: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), barred_luau::CompileError> {
    let cli = Cli::parse();
    let mut config = CompileConfig::default();
    config.mode = if cli.release {
        BuildMode::Release
    } else if cli.debug {
        BuildMode::Debug
    } else {
        BuildMode::Release
    };
    config.anti_tamper.enabled = cli.anti_tamper;
    config.obfuscation.randomize_opcodes = cli.randomize_opcodes;
    config.seed = cli.seed;
    config.encoder.rounds = cli.encoder_rounds;
    config.target = cli.target;

    let input = fs::read_to_string(&cli.input)?;
    let output = barred_luau::compile(&input, &config)?;
    fs::write(&cli.output, output)?;
    Ok(())
}
