use anyhow::anyhow;
use anyhow::Context as _;
use chrono::{DateTime, Utc};

use poise::serenity_prelude as serenity;
use poise::CreateReply;

use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use shuttle_serenity::ShuttleSerenity;

struct Data {
    persist: PersistInstance,
}

type Error = anyhow::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

async fn autocomplete_name<'a>(ctx: Context<'_>, partial: &str) -> Vec<String> {
    if let (Ok(list), Some(guild)) = (ctx.data().persist.list(), ctx.guild_id()) {
        let guild = guild.to_string();
        list.iter()
            .filter(|key| key.starts_with(&guild))
            .map(|key| key.trim_start_matches(&format!("{guild}:")).to_string())
            .filter(|name| name.contains(partial))
            .collect()
    } else {
        Vec::new()
    }
}

/// Create a new event.
///
/// The [text] will display in the message as: "It has been x days since [text]"
#[poise::command(slash_command)]
async fn create(
    ctx: Context<'_>,
    #[description = "Name of the event."] name: String,
    #[description = "Text for the event (e.g. \"It has been x days since [text]\")"] text: String,
) -> Result<(), Error> {
    let guild = ctx.guild_id().context("*Invalid guild")?.to_string();
    let key = format!("{guild}:{name}");

    if let Err(_) = ctx.data().persist.load::<String>(&key) {
        ctx.data()
            .persist
            .save::<(String, DateTime<Utc>)>(&key, (text, Utc::now()))?;

        ctx.say("*Event created.").await?;
        Ok(())
    } else {
        Err(anyhow!("*Event already exists."))
    }
}

/// Update the text for an existing event.
///
/// The [text] will display in the message as: "It has been x days since [text]"
#[poise::command(slash_command)]
async fn update(
    ctx: Context<'_>,
    #[description = "Name of the event."]
    #[autocomplete = "autocomplete_name"]
    name: String,
    #[description = "Text for the event (e.g. \"It has been x days since [text]\")"] text: String,
) -> Result<(), Error> {
    let guild = ctx.guild_id().context("Invalid guild")?.to_string();
    let key = format!("{guild}:{name}");

    if let Ok((_, time)) = ctx.data().persist.load::<(String, DateTime<Utc>)>(&key) {
        ctx.data()
            .persist
            .save::<(String, DateTime<Utc>)>(&key, (text, time))?;
        ctx.say("*Event updated.").await?;
        Ok(())
    } else {
        Err(anyhow!("*Event does not exist."))
    }
}

/// Show the number of days since the last event occurence.
#[poise::command(slash_command)]
async fn days_since(
    ctx: Context<'_>,
    #[description = "Name of the event."]
    #[autocomplete = "autocomplete_name"]
    name: String,
) -> Result<(), Error> {
    let guild = ctx.guild_id().context("Invalid guild")?.to_string();
    let key = format!("{guild}:{name}");

    if let Ok((text, time)) = ctx.data().persist.load::<(String, DateTime<Utc>)>(&key) {
        let days_since = (Utc::now() - time).num_days();
        ctx.say(format!(
            "It has been {} {} since {}.",
            days_since,
            if days_since == 1 { "day" } else { "days" },
            text
        ))
        .await?;
        Ok(())
    } else {
        Err(anyhow!("*Event does not exist."))
    }
}

/// Reset the time since the last event occurence.
#[poise::command(slash_command)]
async fn reset(
    ctx: Context<'_>,
    #[description = "Name of the event."]
    #[autocomplete = "autocomplete_name"]
    name: String,
) -> Result<(), Error> {
    let guild = ctx.guild_id().context("Invalid_guild")?.to_string();
    let key = format!("{guild}:{name}");

    if let Ok((text, _)) = ctx.data().persist.load::<(String, DateTime<Utc>)>(&key) {
        ctx.data()
            .persist
            .save::<(String, DateTime<Utc>)>(&guild, (text.clone(), Utc::now()))?;
        ctx.say(format!("It has now been 0 days since {text}."))
            .await?;
        Ok(())
    } else {
        Err(anyhow!("*Event does not exist."))
    }
}

/// Remove an existing event.
#[poise::command(slash_command)]
async fn remove(
    ctx: Context<'_>,
    #[description = "Name of the event."]
    #[autocomplete = "autocomplete_name"]
    name: String,
) -> Result<(), Error> {
    let guild = ctx.guild_id().context("Invalid_guild")?.to_string();
    let key = format!("{guild}:{name}");

    if let Ok(_) = ctx.data().persist.remove(&key) {
        ctx.say("*Event removed.").await?;
        Ok(())
    } else {
        Err(anyhow!("*Event does not exist."))
    }
}

/// List all existing events.
#[poise::command(slash_command)]
async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().context("Invalid_guild")?.to_string();

    let mut list = String::new();
    for item in ctx.data().persist.list()? {
        if item.starts_with(&guild) {
            let name = item.trim_start_matches(&format!("{guild}:"));
            list.push_str(name);

            let (text, _) = ctx.data().persist.load::<(String, DateTime<Utc>)>(&item)?;
            list.push_str(": ");
            list.push_str(&text);

            list.push('\n');
        }
    }
    list.pop();

    if list.is_empty() {
        ctx.say("*No events found").await?;
    } else {
        ctx.say(format!("*{list}")).await?;
    }

    Ok(())
}

fn maybe_make_ephemeral(_: Context<'_>, create_reply: CreateReply) -> CreateReply {
    if let Some(ref content) = create_reply.content {
        if content.starts_with('*') {
            let new_content = content.trim_start_matches('*').to_string();
            return create_reply.ephemeral(true).content(new_content);
        }
    }

    create_reply
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_runtime::Secrets] secret_store: SecretStore,
    #[shuttle_persist::Persist] persist: PersistInstance,
) -> ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![create(), update(), days_since(), reset(), remove(), list()],
            reply_callback: Some(maybe_make_ephemeral),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { persist })
            })
        })
        .build();

    let client =
        serenity::ClientBuilder::new(discord_token, serenity::GatewayIntents::non_privileged())
            .framework(framework)
            .await
            .map_err(shuttle_runtime::CustomError::new)?;

    Ok(client.into())
}
