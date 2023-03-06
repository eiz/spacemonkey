use crate::db::{self, Database};
use crate::openai::gpt_basic_data;
use anyhow::{anyhow, bail};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serenity::{
    async_trait,
    builder::CreateApplicationCommand,
    model::prelude::{
        command::CommandOptionType,
        interaction::{
            application_command::ApplicationCommandInteraction, Interaction,
            InteractionResponseType,
        },
        GuildId, Ready,
    },
    prelude::{Context, EventHandler, GatewayIntents},
    Client,
};

#[derive(Default, Debug, Clone)]
pub struct BotConfig {
    pub discord_key: String,
    pub openai_key: String,
    pub lambda_key: String,
    pub guild_id: u64,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
struct PromptCommand {
    name: String,
    description: String,
    prompt: String,
}

struct Handler {
    config: BotConfig,
    database: Database,
    prompt_commands: RwLock<Vec<PromptCommand>>,
}

const LAMBDA_PROMPT: &str = "You are SpaceMonkey, a loyal servant of the Imperium of Man. Output a markdown bullet list of available GPU instance types which have regions with available capacity and their total GPU memory, followed by an exhortation of our glorious empire.";
async fn lambda_summary(openai_key: String, lambda_key: String) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let lambda_info = client
        .get("https://cloud.lambdalabs.com/api/v1/instance-types")
        .basic_auth(lambda_key, Some(""))
        .send()
        .await?
        .text()
        .await?;
    gpt_basic_data(openai_key, LAMBDA_PROMPT, lambda_info).await
}

fn configure_prompt_command<'a>(
    prompt_command: &PromptCommand,
    command: &'a mut CreateApplicationCommand,
) -> &'a mut CreateApplicationCommand {
    command
        .name(&prompt_command.name)
        .description(&prompt_command.description)
        .create_option(|o| {
            o.name("input")
                .kind(CommandOptionType::String)
                .description("input to pass to the agent")
                .required(true)
        });
    println!["fuckin wut {:?}", command];
    command
}

fn required_string(cmd: &ApplicationCommandInteraction, name: &str) -> anyhow::Result<String> {
    for opt in &cmd.data.options {
        if opt.name == name {
            return Ok(opt
                .value
                .as_ref()
                .ok_or_else(|| anyhow!("missing required value for {}", name))?
                .as_str()
                .ok_or_else(|| anyhow!("expected string for {} but it wasn't a string", name))?
                .to_owned());
        }
    }

    bail!["missing required value for {}", name];
}

async fn handle_defprompt(
    guild_id: u64,
    ctx: &Context,
    cmd: &ApplicationCommandInteraction,
    database: Database,
    prompt_commands: &RwLock<Vec<PromptCommand>>,
) -> anyhow::Result<String> {
    let new_prompt_command = PromptCommand {
        name: required_string(&cmd, "name")?,
        description: required_string(&cmd, "description")?,
        prompt: required_string(&cmd, "prompt")?,
    };

    GuildId::create_application_command(&GuildId(guild_id), &ctx.http, |command| {
        configure_prompt_command(&new_prompt_command, command)
    })
    .await?;

    let mut prompt_commands = prompt_commands.write();
    prompt_commands.push(new_prompt_command.clone());
    database.set("prompt_commands", &*prompt_commands)?;
    Ok("Command created.".to_owned())
}

async fn handle_lsprompt(
    config: &BotConfig,
    prompt_commands: &RwLock<Vec<PromptCommand>>,
) -> anyhow::Result<String> {
    let prompt_commands_json = serde_json::to_string(&*prompt_commands.read())?;
    let gpt_data = gpt_basic_data(config.openai_key.clone(), "Summarize the following list of commands into a markdown bullet list. Include the prompt text for each.", prompt_commands_json).await?;
    Ok(gpt_data.to_string())
}

async fn handle_prompt_command(
    cmd: &ApplicationCommandInteraction,
    config: &BotConfig,
    prompt_commands: &RwLock<Vec<PromptCommand>>,
) -> anyhow::Result<String> {
    let command = if let Some(command) = prompt_commands
        .read()
        .iter()
        .find(|p| p.name == cmd.data.name)
        .map(|p| p.clone())
    {
        command
    } else {
        bail!("unknown command")
    };

    gpt_basic_data(
        config.openai_key.clone(),
        &command.prompt,
        required_string(&cmd, "input")?,
    )
    .await
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(cmd) = interaction {
            if let Err(e) = cmd
                .create_interaction_response(&ctx.http, |r| {
                    r.kind(InteractionResponseType::DeferredChannelMessageWithSource)
                })
                .await
            {
                eprintln!("Failed to create interaction response {:?}", e);
                return;
            }
            let content = match cmd.data.name.as_str() {
                "lambda" => {
                    lambda_summary(
                        self.config.openai_key.clone(),
                        self.config.lambda_key.clone(),
                    )
                    .await
                }
                "defprompt" => {
                    handle_defprompt(
                        self.config.guild_id,
                        &ctx,
                        &cmd,
                        self.database.clone(),
                        &self.prompt_commands,
                    )
                    .await
                }
                "lsprompt" => handle_lsprompt(&self.config, &self.prompt_commands).await,
                _ => handle_prompt_command(&cmd, &self.config, &self.prompt_commands).await,
            };
            let mut content = content.unwrap_or_else(|e| format!("{:?}", e));
            let mut first_response = true;

            while content.len() > 0 {
                let msg_content = if content.len() > 2000 {
                    let first_2k = content[0..2000].to_owned();
                    content.replace_range(0..2000, "");
                    first_2k
                } else {
                    let tail = content;
                    content = "".to_owned();
                    tail
                };

                let result = if first_response {
                    first_response = false;
                    cmd.edit_original_interaction_response(&ctx.http, |response| {
                        response.content(msg_content)
                    })
                    .await
                } else {
                    cmd.create_followup_message(&ctx.http, |response| response.content(msg_content))
                        .await
                };
                println!("{:?}", result);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("connected {}", ready.user.name);
        let guild_id = GuildId(self.config.guild_id);
        let result = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands
                .create_application_command(|command| {
                    command
                        .name("lambda")
                        .description("get lambdalabs instance type info")
                })
                .create_application_command(|command| {
                    command
                        .name("defprompt")
                        .description("create or update a prompt-based command")
                        .create_option(|o| {
                            o.name("name")
                                .kind(CommandOptionType::String)
                                .description("name of command to create")
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("description")
                                .kind(CommandOptionType::String)
                                .description("description of the command")
                                .required(true)
                        })
                        .create_option(|o| {
                            o.name("prompt")
                                .kind(CommandOptionType::String)
                                .description("prompt to use")
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command
                        .name("rmprompt")
                        .description("delete a prompt-based command")
                        .create_option(|o| {
                            o.name("name")
                                .kind(CommandOptionType::String)
                                .description("name of command to delete")
                                .required(true)
                        })
                })
                .create_application_command(|command| {
                    command
                        .name("lsprompt")
                        .description("list a summary of saved prompts")
                });

            for prompt_command in self.prompt_commands.read().iter() {
                commands.create_application_command(|command| {
                    configure_prompt_command(prompt_command, command)
                });
            }

            commands
        })
        .await;

        println!("{:?}", result);
    }
}

pub async fn run(config: BotConfig, database: db::Database) -> anyhow::Result<()> {
    let mut client = Client::builder(&config.discord_key, GatewayIntents::empty())
        .event_handler(Handler {
            config,
            prompt_commands: RwLock::new(database.get("prompt_commands").unwrap_or_default()),
            database: database,
        })
        .await
        .expect("error creating client");
    if let Err(e) = client.start().await {
        println!("Client error: {:?}", e);
    }
    Ok(())
}
