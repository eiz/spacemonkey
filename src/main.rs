use std::{fs, path::PathBuf};

use anyhow::anyhow;
use bot::BotConfig;
use clap::{Args, Parser, Subcommand};

mod bot;
mod db;
mod openai;
mod train_extract;

#[derive(Subcommand, Debug)]
enum Command {
    Bot(BotArgs),
    GenerateQuestions(GenerateQuestionsArgs),
}

#[derive(Args, Debug)]
struct BotArgs {
    #[arg(long, help = "discord guild id")]
    guild_id: u64,
}

#[derive(Args, Debug)]
struct GenerateQuestionsArgs {
    #[arg(long, help = "input list of topics one per line")]
    topics: PathBuf,
    #[arg(long, help = "output directory")]
    out_dir: PathBuf,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct GlobalArgs {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = GlobalArgs::parse();
    let home = dirs::home_dir().ok_or_else(|| anyhow!["missing $HOME"])?;
    let openai_key = fs::read_to_string(home.join(".openai"))?.trim().to_owned();
    let discord_key = fs::read_to_string(home.join(".discord"))?.trim().to_owned();
    let lambda_key = fs::read_to_string(home.join(".lambda"))?.trim().to_owned();
    let database = db::open()?;

    match args.command {
        Command::Bot(args) => {
            let config = BotConfig {
                discord_key,
                lambda_key,
                openai_key,
                guild_id: args.guild_id,
            };
            if let Err(e) = bot::run(config, database).await {
                eprintln!("Bot error {:?}", e);
            }
        }

        Command::GenerateQuestions(args) => {
            train_extract::extract_topic_questions(openai_key, args.topics, args.out_dir).await?
        }
    }

    Ok(())
}
