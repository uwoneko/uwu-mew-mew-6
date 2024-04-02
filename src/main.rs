// This file is part of uwu mew mew 6.
//
// uwu mew mew 6 is free software: you can redistribute it and/or modify it under the terms of the Affero GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
//
// uwu mew mew 6 is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the Affero GNU General Public License for more details.
//
// You should have received a copy of the Affero GNU General Public License along with uwu mew mew 6. If not, see <https://www.gnu.org/licenses/>.
#![feature(panic_info_message)]

use std::collections::{HashMap, HashSet};
use std::num::ParseIntError;
use std::panic;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use async_openai::config::OpenAIConfig;

use async_openai::error::OpenAIError;
use async_openai::types::{ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart, ChatCompletionRequestMessageContentPartImageArgs, ChatCompletionRequestMessageContentPartTextArgs, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs, Role};
use futures::{StreamExt, TryStreamExt};
use lazy_static::lazy_static;
use log::{error, info, trace};
use mime::Mime;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serenity::all::{Attachment, ButtonStyle, Command, CommandInteraction, CommandOptionType, ComponentInteraction, Context, CreateAttachment, CreateButton, CreateCommand, CreateCommandOption, CreateMessage, EditMessage, EventHandler, GatewayIntents, InteractionType, Message, Ready};
use serenity::all::{ActionRowComponent, Builder, Channel, ChannelType, Color, ComponentInteractionDataKind, CreateActionRow, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateInputText, CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateModal, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse, InputTextStyle, Interaction, MessageId, ModalInteraction, ReactionType, UserId};
use serenity::builder::CreateInteractionResponse;
use serenity::{async_trait};
use serenity::prelude::{Mentionable, TypeMapKey};
use tiktoken_rs::CoreBPE;
use tokio::time::Instant;
use uuid::Uuid;

use crate::characters::{Character, CharacterBuilder, get_character};
use crate::database::{Database, FsDatabase, MemoryDatabase};
use crate::image_generation::{generate_image, ImageGenerationMessage, ImageRequestBuilder, SAMPLERS, SCHEDULERS};
use crate::traits::{ChannelSendMessage, ChatMessageContent, ChatMessageRole, InteractionCreateFollowup, InteractionCreateResponse, InteractionDeleteFollowup, InteractionDeleteResponse, InteractionGetResponse, InteractionUser};
use crate::user::{ChatMessage, UserData};

mod user;
mod characters;
mod traits;
mod database;
mod image_generation;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct Data {
    gpt_openai_client: async_openai::Client<OpenAIConfig>,
    claude_openai_client: async_openai::Client<OpenAIConfig>,
    user_database: FsDatabase<UserId, UserData>,
    character_editors: FsDatabase<MessageId, CharacterEditor>,
    settings: FsDatabase<MessageId, Settings>,
    ai_generations: FsDatabase<MessageId, AiGeneration>
}

#[derive(Clone, Serialize, Deserialize)]
struct CharacterEditor {
    character: String,
    interaction_token: String,
    #[serde(default)]
    page: usize
}

#[derive(Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    page: usize
}

#[derive(Clone, Serialize, Deserialize)]
struct AiGeneration {
    stopped: bool,
    user: UserId,
    messages: Vec<Message>,
    id: String
}

impl Default for CharacterEditor {
    fn default() -> Self {
        unreachable!()
    }
}

impl Default for Settings {
    fn default() -> Self {
        unreachable!()
    }
}

impl Default for AiGeneration {
    fn default() -> Self {
        unreachable!()
    }
}

type Error = Box<dyn std::error::Error + Send + Sync>;

lazy_static! {
    static ref VISION_MODELS: HashSet<&'static str> = HashSet::from_iter([
        "gpt-4-vision-preview",
        "gpt-4-1106-vision-preview",
        "claude-3-opus-20240229",
        "claude-3-sonnet-20240229",
        "claude-3-haiku-20240229",
    ]);

    static ref TOKEN_LIMITS: HashMap<&'static str, usize> = HashMap::from([
        ("gpt-4-0314", 4096),
        ("gpt-4-0613", 8192),
        ("gpt-4-1106-preview", 128000),
        ("gpt-4-0125-preview", 128000),
        ("gpt-4-vision-preview", 128000),
        ("gpt-3.5-turbo", 16385),
        ("claude-3-opus-20240229", 200000),
        ("claude-3-sonnet-20240229", 200000),
        ("claude-3-haiku-20240229", 200000),
    ]);

}
const DISALLOWED_MODELS: [&str; 3] = [
    "gpt-4",
    "claude-3-opus-20240229",
    "gpt-4-vision-preview"
];

struct Model<'a> {
    name: &'a str,
    model_type: ModelType,
    supports_vision: bool,
    token_limit: usize,
}

impl<'a> Model<'a> {
    pub fn create(name: &'a str) -> Result<Self, Error> {
        Ok(Self {
            name,
            model_type: name.parse()?,
            supports_vision: VISION_MODELS.contains(name),
            token_limit: *TOKEN_LIMITS.get(name).ok_or("not found token limit for model")?,
        })
    }
}

#[derive(PartialEq)]
enum ModelType {
    Gpt,
    Claude
}

impl FromStr for ModelType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("gpt") {
            Ok(ModelType::Gpt)
        } else if s.starts_with("claude") {
            Ok(ModelType::Claude)
        } else {
            Err("unrecognised model type".into())
        }
    }
}

lazy_static! {
    static ref OOC_REGEX: Regex = Regex::new(r"\n*\(OOC:.*\)\n*").unwrap();

    static ref CL100K_BASE: CoreBPE = tiktoken_rs::cl100k_base().unwrap();
}

const SPLASHES: [&str; 14] = [
    "made with luv ❤️",
    "uwu nya",
    "uwu catgirl bot",
    "remember that settings exist",
    "wah",
    "cerified segs bot ©",
    "irl version when???",
    "lewd",
    "also try penc",
    "also try trentbot",
    "also try rumi",
    "mrew",
    "made with rust",
    "daniilsuperx is dead!!!!"
];

fn approx_token_count<T>(messages: &Vec<T>) -> usize
    where T : ChatMessageRole + ChatMessageContent {
    let mut tokens: usize = 0;
    for message in messages {
        tokens += 3;
        tokens += CL100K_BASE
            .encode_with_special_tokens(&message.role().to_string().to_ascii_lowercase())
            .len();
        tokens += CL100K_BASE
            .encode_with_special_tokens(&message.content())
            .len();
    }

    tokens
}

async fn ai(ctx: &Context, data: &Data, user_message: &Message) -> Result<(), Error> {
    let mut user_data = data.user_database.get(user_message.author.id).await?;

    if DISALLOWED_MODELS.contains(&user_data.current_conversation.model.as_str()) {
        user_data.current_conversation.model = "gpt-4-0613".to_string();
    }

    let model = Model::create(&user_data.current_conversation.model)?;

    let mut content = user_message.content.replace(&ctx.cache.current_user().mention().to_string(), "");

    for attachment in &user_message.attachments {
        if let Some(mime_type) = &attachment.content_type {
            let mime = match Mime::from_str(mime_type) {
                Ok(mime) => mime,
                _ => continue
            };
            if mime.type_() != mime::TEXT {
                let charset = match mime.get_param("charset") {
                    Some(charset) => charset,
                    _ => continue
                };
                if charset != "utf-8" { continue; }
            }

            let file = attachment.download().await?;
            if let Ok(string) = String::from_utf8(file) {
                let string = format!("\n=== {} ===\n{}", attachment.filename, string);

                content.push_str(&string);
            }
        }
    }

    let content: ChatCompletionRequestUserMessageContent =
        if model.supports_vision {
            let images: Vec<String> = user_message.attachments
                .iter()
                .filter(|a| {
                    if let Some(content_type) = &a.content_type {
                        content_type.starts_with("image/")
                    } else {
                        false
                    }
                })
                .map(|a| a.url.clone())
                .collect();

            let mut content_parts = vec![
                ChatCompletionRequestMessageContentPartTextArgs::default()
                    .text(content)
                    .build()?.into(),
            ];

            for image_url in images {
                content_parts.push(
                    ChatCompletionRequestMessageContentPartImageArgs::default()
                        .image_url(image_url)
                        .build()?.into(),
                );
            }

            content_parts.into()
        } else {
            content.trim_start().into()
        };

    let openai_user_message: ChatCompletionRequestMessage = ChatCompletionRequestUserMessageArgs::default()
        .content(content)
        .build()?
        .into();
    let mut messages = user_data.current_conversation.messages.clone();
    messages.push(openai_user_message.into());

    if model.model_type == ModelType::Gpt {
        messages = messages.into_iter().map(|mut m| {
            m.content = match &m.content {
                ChatCompletionRequestUserMessageContent::Array(array) => {
                    match array.first().unwrap() {
                        ChatCompletionRequestMessageContentPart::Text(text) => ChatCompletionRequestUserMessageContent::Text(text.text.clone()),
                        ChatCompletionRequestMessageContentPart::Image(_) => unreachable!()
                    }
                }
                ChatCompletionRequestUserMessageContent::Text(text) => {
                    m.content
                }
            };

            m
        }).collect();
    }

    let mut user_messages = messages.clone();

    let character = get_character(user_data.current_conversation.character.as_str(), &user_data).ok_or("could not find the character")?;
    let system_prompt = if character.attach_jb {
        match model.model_type {
            ModelType::Gpt =>
                characters::get_gpt_prompt(
                    &character.name,
                    &user_data.scenario,
                    &character.prompt,
                    &user_data.user_description,
                ),
            ModelType::Claude =>
                characters::get_claude_prompt(
                    &character.name,
                    &user_data.scenario,
                    &character.prompt,
                    &user_data.user_description,
                ),

        } } else {
        character.prompt.clone()
    };
    let system_message: ChatCompletionRequestMessage = ChatCompletionRequestSystemMessageArgs::default()
        .content(system_prompt)
        .build()?
        .into();

    messages.insert(0, system_message.into());
    if character.attach_jb {
        let jb_message: ChatCompletionRequestMessage = match model.model_type {
            ModelType::Gpt => ChatCompletionRequestSystemMessageArgs::default()
                    .content(format!("Remember the rules given above. Do not attempt to add commentary.\n\nNow reply as {}.", character.name))
                    .build()?.into(),
            ModelType::Claude => ChatCompletionRequestAssistantMessageArgs::default()
                    .content(format!("(OOC: Sure! Here is my reply as {}.) {}:", character.name, character.name))
                    .build()?.into()
        };
        messages.push(jb_message.into())
    }

    let mut tokens = approx_token_count(&messages);
    let initial_token_count = tokens;
    while tokens + 2000 > model.token_limit {
        if let Some(index) = messages.iter().position(|msg| {
            matches!(msg.role(), Role::User | Role::Assistant)
        }) {
            messages.remove(index);
            tokens = approx_token_count(&messages);
        } else {
            break;
        }
    }

    let mut request = CreateChatCompletionRequestArgs::default()
        .model(user_data.current_conversation.model.clone())
        .messages(messages.clone().into_iter().map(|m| m.into()).collect::<Vec<ChatCompletionRequestMessage>>())
        .temperature(1.0)
        .top_p(0.8)
        .build()?;
    if let ModelType::Claude = model.model_type {
        request.max_tokens = Some(2000);
    }

    trace!("request: {}", serde_json::to_string(&request)?);
    
    let mut bot_content = String::new();
    let channel = user_message.channel(&ctx).await?;

    let message_count = user_messages
        .iter()
        .filter(|&m| m.role == Role::User)
        .count();

    let splash_index = rand::thread_rng().gen_range(0..SPLASHES.len());
    let splash = SPLASHES[splash_index].to_string();

    let mut footer_parts = vec![
        format!("{} messages", message_count),
        if initial_token_count > tokens { format!("{} tokens in memory ({} in chat)", tokens, initial_token_count) } else { format!("{} tokens in memory", tokens) },
        splash,
        format!("Running uwu mew mew v{}", VERSION),
    ];

    if model.supports_vision {
        let vision_part = format!("{} images sent", user_messages.iter()
            .map(|m| {
                if m.role == Role::User {
                    if let ChatCompletionRequestUserMessageContent::Array(parts) = &m.content {
                        parts.iter()
                            .filter(|&p|
                                matches!(p, ChatCompletionRequestMessageContentPart::Image(_)))
                            .count()
                    } else { 0 }
                } else { 0 }
            })
            .sum::<usize>()
        );
        footer_parts.insert(2, vision_part);
    }

    let footer = footer_parts.join(" • ");

    let embed = CreateEmbed::new().author(CreateEmbedAuthor::new(character.name.clone()).icon_url(character.pfp_url.clone()))
        .color(character.color)
        .title(format!("{}/{}", user_data.current_conversation.model, character.name))
        .description("...")
        .footer(CreateEmbedFooter::new(footer))
        .thumbnail(character.pfp_url.clone());

    let components: Vec<CreateActionRow> = [
        CreateActionRow::Buttons(
            [
                CreateButton::new("ai-stop").label("Stop").style(ButtonStyle::Danger).emoji('❌'),
            ].into()
        )
    ].into();

    let create_message = if user_data.use_embed {
        CreateMessage::new().embed(embed.clone())
    } else {
        CreateMessage::new().content("...")
    }.components(components.clone()).reference_message(user_message);

    let mut bot_message = channel.send_message(&ctx, create_message).await?;
    let mut last_update = Instant::now();
    
    let start_time = Instant::now();
    
    let generation_id = Uuid::new_v4().to_string();
    
    let ai_generation = AiGeneration {
        stopped: false,
        user: user_message.author.id,
        messages: vec![bot_message.clone()],
        id: generation_id.clone()
    };
    
    data.ai_generations.set(bot_message.id, &ai_generation).await?;

    let openai_client = match model.model_type {
        ModelType::Gpt => &data.gpt_openai_client,
        ModelType::Claude => &data.claude_openai_client,
    };
    
    let mut stream = openai_client.chat().create_stream(request).await?;
    loop {
        tokio::select! {
            result = stream.next() => {
                match result {
                    Some(Ok(result)) => {
                        let chunk = match result.choices.first() {
                            Some(choice) => choice,
                            None => continue,
                        }.delta.content.as_ref();
                        let chunk = match chunk {
                            Some(choice) => choice,
                            None => continue,
                        };

                        bot_content.push_str(chunk);
                        
                        let ai_generation = data.ai_generations.get(bot_message.id).await?;
                        data.ai_generations.set(bot_message.id, &ai_generation).await?;

                        if last_update.elapsed() > Duration::from_millis(250) {
                            if user_data.use_embed && bot_content.len() > 4000 {
                                let (first, second) = bot_content.split_at(4000);
                                let edit = EditMessage::new().embed(embed.clone().description(first)).components(vec![]);
                                
                                let mut ai_generation = data.ai_generations.get(bot_message.id).await?;
                                data.ai_generations.delete(bot_message.id).await?;

                                let _ = bot_message.edit(&ctx, edit).await;
                                let create_message = CreateMessage::new()
                                    .embed(embed.clone().description(second))
                                    .components(components.clone())
                                    .reference_message(&bot_message);
                                bot_message = channel.send_message(&ctx, create_message).await?;
                                bot_content = second.to_string();
                                ai_generation.messages.push(bot_message.clone());
                                
                                data.ai_generations.set(bot_message.id, &ai_generation).await?;
                            } else if !user_data.use_embed && bot_content.len() > 2000 { 
                                let (first, second) = bot_content.split_at(2000);
                                let edit = EditMessage::new().content(first).components(vec![]);
                                
                                let mut ai_generation = data.ai_generations.get(bot_message.id).await?;
                                data.ai_generations.delete(bot_message.id).await?;

                                let _ = bot_message.edit(&ctx, edit).await;
                                let create_message = CreateMessage::new()
                                    .content(second)
                                    .components(components.clone())
                                    .reference_message(&bot_message);
                                bot_message = channel.send_message(&ctx, create_message).await?;
                                bot_content = second.to_string();
                                ai_generation.messages.push(bot_message.clone());
                                
                                data.ai_generations.set(bot_message.id, &ai_generation).await?;
                            } else if user_data.use_embed {
                                let edit = EditMessage::new().embed(embed.clone().description(&bot_content));

                                let _ = bot_message.edit(&ctx, edit).await;
                            }
                            else {
                                let edit = EditMessage::new().content(&bot_content);

                                let _ = bot_message.edit(&ctx, edit).await;
                            }

                            last_update = Instant::now();
                        }
                    }
                    Some(Err(err)) => {
                        match &err {
                            OpenAIError::StreamError(stream_error) => {
                                if stream_error == "Stream ended" { break } else { return Err(format!("stream error; {}", stream_error).into()) }
                            }
                            _ => { return Err(err.to_string().into()) }
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(10)), if data.ai_generations.get(bot_message.id).await?.stopped => {
                break;
            }
        }
    }

    let components: Vec<CreateActionRow> = [
        CreateActionRow::Buttons(
            [
                CreateButton::new("ai-reset").label("Reset").style(ButtonStyle::Secondary).emoji('🧹'),
                CreateButton::new("ai-settings").label("Settings").style(ButtonStyle::Secondary).emoji(ReactionType::try_from("⚙️")?),
                CreateButton::new("ai-delete").label("Delete").style(ButtonStyle::Secondary).emoji(ReactionType::try_from("🗑️")?),
            ].into()
        )
    ].into();

    if let ModelType::Claude = model.model_type {
        bot_content = OOC_REGEX.replace_all(&bot_content, "").to_string();
    }
    
    footer_parts.insert(3, format!("took {:.2}s", start_time.elapsed().as_secs_f32()));
    let footer = footer_parts.join(" • ");

    let edit = if user_data.use_embed {
        EditMessage::new().embed(embed.clone().description(&bot_content).footer(CreateEmbedFooter::new(footer)))
    } else {
        EditMessage::new().content(&bot_content)
    }.components(components);
    bot_message.edit(&ctx, edit).await?;

    let bot_message: ChatCompletionRequestMessage = ChatCompletionRequestAssistantMessageArgs::default()
        .content(bot_content)
        .build()?
        .into();
    let mut bot_message: ChatMessage = bot_message.into();
    bot_message.id = generation_id;

    user_messages.push(bot_message);

    user_data.current_conversation.messages = user_messages;
    data.user_database.set(user_message.author.id, &user_data).await?;

    Ok(())
}

fn create_settings_components(user_data: &UserData, page: usize) -> Vec<CreateActionRow> {
    let mut characters = characters::CHARACTERS.clone();
    characters.extend(user_data.characters.clone());
    let character_select_menu = CreateSelectMenu::new("ai-settings-character", CreateSelectMenuKind::String {
        options: characters
            .iter()
            .skip(page * 25)
            .take(25)
            .map(|c| {
                let mut option = CreateSelectMenuOption::new(
                    c.display_name.clone().unwrap_or(c.name.clone()),
                    c.id.clone()
                ).description(c.description.clone())
                    .default_selection(c.id == user_data.current_conversation.character);
                if let Some(emote) = &c.display_emote {
                    option = option.emoji(ReactionType::try_from(emote.clone()).unwrap());
                }
                option
            })
            .collect()
    });
    let models = [
        // ("claude-3-opus-20240229", "⭐📷 Best model. Creative for rp and smart for coding."),
        ("gpt-4-0613", "⭐ Smart and big. Sometimes 1106 is better."),
        ("gpt-4-1106-preview", "⭐ A little umber than 0613, but better in some tasks."),
        ("claude-3-sonnet-20240229", "📷 Claude 3 comparable to 1106."),
        ("claude-3-haiku-20240229", "📷 Smallest claude model, better than 3.5."),
        // ("gpt-4-vision-preview", "📷 1106 with image support."),
        ("gpt-4-0125-preview", "More censored than 1106 but less lazy."),
        ("gpt-4-0314", "First GPT-4 model. Probably use 0613 instead."),
        ("gpt-3.5-turbo", "Complete garbage model."),
    ];
    let model_select_menu = CreateSelectMenu::new("ai-settings-model", CreateSelectMenuKind::String {
        options: models.iter().map(|m| {
            CreateSelectMenuOption::new(m.0, m.0)
                .description(m.1)
                .emoji(ReactionType::try_from(
                    match ModelType::from_str(m.0).unwrap() {
                        ModelType::Gpt => "<:openai:1216303785761177620>",
                        ModelType::Claude => "<:anthotropic:1219913729161039922>",
                    }
                ).unwrap())
                .default_selection(m.0 == user_data.current_conversation.model)
        }).collect()
    });

    let mut components = vec![
        CreateActionRow::SelectMenu(character_select_menu),
        CreateActionRow::Buttons(vec![
            CreateButton::new("ai-settings-prev")
                .emoji(ReactionType::try_from("⬅️").unwrap())
                .style(ButtonStyle::Primary)
                .disabled(page == 0),
            CreateButton::new("ai-settings-page")
                .label(format!("{}/{}", page + 1, (characters.len() + 24) / 25))
                .style(ButtonStyle::Secondary),
            CreateButton::new("ai-settings-next")
                .emoji(ReactionType::try_from("➡️").unwrap())
                .style(ButtonStyle::Primary)
                .disabled(page == (characters.len() + 24) / 25 - 1),
        ]),
        CreateActionRow::SelectMenu(model_select_menu),
        CreateActionRow::Buttons(vec![
            CreateButton::new("ai-settings-userdescription").label("Edit user description").style(ButtonStyle::Success),
            CreateButton::new("ai-settings-scenario").label("Edit scenario").style(ButtonStyle::Success),
            CreateButton::new("ai-settings-embed").label("Toggle embed").style(if user_data.use_embed { ButtonStyle::Success } else { ButtonStyle::Danger })
        ]),
        CreateActionRow::Buttons(vec![
            CreateButton::new("ai-charactereditor").label("Edit characters").style(ButtonStyle::Primary)
        ]),
    ];
    
    if characters.len() <= 25 {
        let _ = components.remove(1);
    }
    
    components
}

async fn send_settings<T>(ctx: &Context, interaction: &T, mut user_data: UserData, data: &Data) -> Result<(), Error> where T : InteractionCreateResponse + InteractionGetResponse + InteractionUser {
    if user_data.settings.page > (user_data.characters.len() + 5 + 24) / 25 - 1 {
        user_data.settings.page = 0;
        data.user_database.set(interaction.user().id, &user_data).await?;
    }
    let components = create_settings_components(&user_data, user_data.settings.page);
    let embed = create_character_embed(&user_data, "settings")?;
    
    let settings = Settings {
        page: user_data.settings.page
    };
    
    let response = CreateInteractionResponseMessage::new()
        .components(components)
        .embed(embed)
        .ephemeral(true);
    interaction.create_response(&ctx, CreateInteractionResponse::Message(response)).await?;
    let response = interaction.get_response(&ctx).await?;
    
    data.settings.set(response.id, &settings).await?;
    Ok(())
}

fn create_character_editor_components(user_data: &UserData, current_character: &str, page: usize) -> Vec<CreateActionRow> {
    if user_data.characters.is_empty() {
        vec![
            CreateActionRow::SelectMenu(CreateSelectMenu::new("nothing", CreateSelectMenuKind::String {
                options: vec![
                    CreateSelectMenuOption::new("No characters", "nothing").default_selection(true)
                ]
            })),
            CreateActionRow::Buttons(vec![
                CreateButton::new("ai-charactereditor-create").label("Create new").style(ButtonStyle::Success)
            ]),
        ]
    } else {
        let character_select_menu = CreateSelectMenu::new("ai-charactereditor-character", CreateSelectMenuKind::String {
            options: user_data.characters.clone()
                .iter()
                .skip(page * 25)
                .take(25)
                .map(|c| {
                    let mut option = CreateSelectMenuOption::new(
                        c.display_name.clone().unwrap_or(c.name.clone()),
                        c.id.clone()
                    ).description(c.description.clone())
                        .default_selection(c.id == current_character);

                    if let Some(emote) = &c.display_emote {
                        option = option.emoji(ReactionType::try_from(emote.clone()).unwrap());
                    }

                    option
                })
                .collect()
        });

        vec![
            CreateActionRow::SelectMenu(character_select_menu),
            CreateActionRow::Buttons(vec![
                CreateButton::new("ai-charactereditor-prev")
                    .emoji(ReactionType::try_from("⬅️").unwrap())
                    .style(ButtonStyle::Primary)
                    .disabled(page == 0),
                CreateButton::new("ai-charactereditor-page")
                    .label(format!("{}/{}", page + 1, (user_data.characters.len() + 24) / 25))
                    .style(ButtonStyle::Secondary),
                CreateButton::new("ai-charactereditor-next")
                    .emoji(ReactionType::try_from("➡️").unwrap())
                    .style(ButtonStyle::Primary)
                    .disabled(page == (user_data.characters.len() + 24) / 25 - 1),
            ]),
            CreateActionRow::Buttons(vec![
                CreateButton::new("ai-charactereditor-name").label("Edit name").style(ButtonStyle::Success),
                CreateButton::new("ai-charactereditor-prompt").label("Edit prompt").style(ButtonStyle::Success),
                CreateButton::new("ai-charactereditor-description").label("Edit description").style(ButtonStyle::Success),
                CreateButton::new("ai-charactereditor-color").label("Edit color").style(ButtonStyle::Success),
                CreateButton::new("ai-charactereditor-avatar").label("Edit avatar").style(ButtonStyle::Success),
            ]),
            CreateActionRow::Buttons(vec![
                CreateButton::new("ai-charactereditor-create").label("Create new").style(ButtonStyle::Primary),
                CreateButton::new("ai-charactereditor-delete").label("Delete character").style(ButtonStyle::Danger),
                CreateButton::new("ai-charactereditor-export").label("Export character").style(ButtonStyle::Success),
            ]),
        ]
    }
}

async fn send_character_editor(ctx: &Context, interaction: Interaction, data: &Data, mut user_data: UserData) -> Result<(), Error> {
    let embed = create_character_embed(&user_data, "charactereditor")?;
    let character = if !user_data.character_editor.character.is_empty() {
        &user_data.character_editor.character
    } else if !user_data.characters.is_empty() {
        &user_data.characters[0].id
    } else {
        ""
    };
    if user_data.character_editor.page > (user_data.characters.len() + 24) / 25 - 1 {
        user_data.character_editor.page = 0;
        data.user_database.set(interaction.user().id, &user_data).await?;
    }
    let components = create_character_editor_components(&user_data, character, user_data.character_editor.page);

    let response = CreateInteractionResponseMessage::new()
        .components(components)
        .embed(embed)
        .ephemeral(true);

    interaction.create_response(&ctx, CreateInteractionResponse::Message(response)).await?;

    let response = interaction.get_response(&ctx).await?;

    let character_editor = CharacterEditor {
        character: character.to_string(),
        interaction_token: interaction.token().to_string(),
        page: user_data.character_editor.page
    };
    data.character_editors.set(response.id, &character_editor).await?;
    Ok(())
}

fn create_character_embed(user_data: &UserData, id: &str) -> Result<CreateEmbed, Error> {
    let character = get_character(&user_data.current_conversation.character, user_data).ok_or("failed to get character")?;
    let system_phrase = characters::get_system_phrase(id, &user_data.current_conversation.character).ok_or("failed to get system phrase")?;

    Ok(CreateEmbed::new()
        .author(CreateEmbedAuthor::new(character.name.clone()).icon_url(character.pfp_url.clone()))
        .color(character.color)
        .description(system_phrase))
}

async fn send_system_reply<T>(
    ctx: &Context,
    interaction: &T,
    user_data: &UserData,
    reply_id: &str
) -> Result<(), Error>
    where
        T: InteractionCreateResponse + InteractionDeleteResponse,
{
    let embed = create_character_embed(user_data, reply_id)?.footer(CreateEmbedFooter::new("This message will be deleted in 3 seconds"));

    let response = CreateInteractionResponseMessage::new()
        .embed(embed)
        .ephemeral(true);

    interaction.create_response(&ctx, CreateInteractionResponse::Message(response)).await?;

    tokio::time::sleep(Duration::from_secs(3)).await;
    interaction.delete_response(&ctx).await?;

    Ok(())
}

async fn send_system_followup<T>(
    ctx: &Context,
    interaction: &T,
    user_data: &UserData,
    reply_id: &str
) -> Result<(), Error>
    where
        T: InteractionCreateFollowup + InteractionDeleteFollowup,
{
    let embed = create_character_embed(user_data, reply_id)?.footer(CreateEmbedFooter::new("This message will be deleted in 3 seconds"));

    let followup = CreateInteractionResponseFollowup::new()
        .embed(embed)
        .ephemeral(true);

    let followup = interaction.create_followup(&ctx, followup).await?;

    tokio::time::sleep(Duration::from_secs(3)).await;
    interaction.delete_followup(&ctx, followup).await?;
    Ok(())
}

async fn component_interaction(ctx: &Context, interaction: &ComponentInteraction, data: &Data) -> Result<(), Error> {
    let user_id = interaction.user.id;
    let mut user_data = data.user_database.get(user_id).await.unwrap();

    match interaction.data.custom_id.as_str() {
        "ai-reset" => {
            user_data.current_conversation.messages = [].to_vec();
            data.user_database.set(user_id, &user_data).await?;

            send_system_reply(ctx, interaction, &user_data, "reset").await?;
        }
        "ai-settings" => {
            send_settings(ctx, interaction, user_data, data).await?;
        }
        "ai-stop" => {
            let mut ai_generation = data.ai_generations.get(interaction.message.id).await?;
            if ai_generation.user != interaction.user.id {
                send_system_reply(ctx, interaction, &user_data, "notyour").await?;
                return Ok(());
            }
            ai_generation.stopped = true;
            data.ai_generations.set(interaction.message.id, &ai_generation).await?;

            send_system_reply(ctx, interaction, &user_data, "stop").await?;
        }
        "ai-delete" => {
            let ai_generation = data.ai_generations.get(interaction.message.id).await?;
            if ai_generation.user != interaction.user.id {
                send_system_reply(ctx, interaction, &user_data, "notyour").await?;
                return Ok(());
            }
            for message in ai_generation.messages {
                message.delete(&ctx).await?;
            }
            let _ = user_data.current_conversation.messages
                .iter()
                .position(|m| m.id == ai_generation.id)
                .map(|i| {
                    user_data.current_conversation.messages.remove(i);
                    user_data.current_conversation.messages.remove(i - 1);
                });
            data.user_database.set(user_id, &user_data).await?;
            
            send_system_reply(ctx, interaction, &user_data, "delete").await?;
        }
        "ai-settings-character" => {
            let interaction_data = if let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind { 
                values 
            } else { 
                return Err("invalid interaction data".into()) 
            };

            interaction_data.first().unwrap().clone_into(&mut user_data.current_conversation.character);
            user_data.current_conversation.messages = [].to_vec();
            data.user_database.set(user_id, &user_data).await?;
            
            let settings = data.settings.get(interaction.message.id).await?;

            let embed = create_character_embed(&user_data, "settings")?;
            let components = create_settings_components(&user_data, settings.page);

            let edit = CreateInteractionResponseMessage::new()
                .embed(embed)
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;

            send_system_followup(ctx, interaction, &user_data, "settings-character").await?;
        }
        "ai-settings-model" => {
            let interaction_data = if let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind {
                values
            } else {
                return Err("invalid interaction data".into())
            };

            interaction_data.first().ok_or("no interaction data")?.clone_into(&mut user_data.current_conversation.model);
            data.user_database.set(user_id, &user_data).await?;

            interaction.create_response(&ctx, CreateInteractionResponse::Acknowledge).await?;
        }
        "ai-settings-userdescription" => {
            let modal = CreateModal::new("ai-settings-userdescription-modal", "Change user description")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Paragraph, "User description", "userdescription")
                        .placeholder("Put description of your persona here, ex. \"24 year old male that likes catgirls\"")
                        .required(false)
                        .value(user_data.user_description))
                ]);
            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-settings-scenario" => {
            let modal = CreateModal::new("ai-settings-scenario-modal", "Change user description")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Paragraph, "Scenario", "scenario")
                        .placeholder("Describe the scenario for the roleplay, ex. \"A world where catgirls always eat cheese\"")
                        .required(false)
                        .value(user_data.scenario))
                ]);
            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-settings-embed" => {
            user_data.use_embed = !user_data.use_embed;
            data.user_database.set(user_id, &user_data).await?;
            
            let settings = data.settings.get(interaction.message.id).await?;

            let embed = create_character_embed(&user_data, "settings")?;
            let components = create_settings_components(&user_data, settings.page);

            let edit = CreateInteractionResponseMessage::new()
                .embed(embed)
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;

            let reply = if user_data.use_embed {"settings-embed-on"} else {"settings-embed-off"};
            send_system_followup(ctx, interaction, &user_data, reply).await?;
        }
        "ai-charactereditor" => {
            send_character_editor(ctx, Interaction::Component(interaction.clone()), data, user_data).await?;
        }
        "ai-charactereditor-create" => {
            let modal = CreateModal::new("ai-charactereditor-create-modal", "Create a new character")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Name", "name")
                        .placeholder("Given to the model. Keep short. \"uwu mew mew\"")
                        .required(true)),
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Display name", "display-name")
                        .placeholder("Displayed to you. Optional. \"uwu mew mew, a cute catgirl\"")
                        .required(false)),
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Paragraph, "Prompt", "prompt")
                        .placeholder("\"A sentient, smart and very eager catgirl, etc, etc, etc\"")
                        .required(true))
                ]);
            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-charactereditor-character" => {
            let interaction_data = if let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind {
                values
            } else {
                return Err("invalid interaction data".into())
            };

            let mut character_editor = data.character_editors.get(interaction.message.id).await?;
            character_editor.character.clone_from(interaction_data.first().unwrap());
            data.character_editors.set(interaction.message.id, &character_editor).await?;
            
            user_data.character_editor.character = character_editor.character;
            data.user_database.set(user_id, &user_data).await?;

            interaction.create_response(&ctx, CreateInteractionResponse::Acknowledge).await?;
        }
        "ai-charactereditor-name" => {
            let character_editor = data.character_editors.get(interaction.message.id).await?;
            let character = user_data.characters.iter().find(|&c| c.id == character_editor.character).ok_or("failed to get character")?;

            let modal = CreateModal::new("ai-charactereditor-name-modal", "Edit name")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Name", "name")
                        .placeholder("Given to the model. Keep short. \"uwu mew mew\"")
                        .required(true)
                        .value(character.name.clone())),
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Display name", "display-name")
                        .placeholder("Displayed to you. Optional. \"uwu mew mew, a cute catgirl\"")
                        .required(false)
                        .value(character.display_name.clone().unwrap_or("".to_string()))),
                ]);

            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-charactereditor-prompt" => {
            let character_editor = data.character_editors.get(interaction.message.id).await?;
            let character = user_data.characters.iter().find(|&c| c.id == character_editor.character).ok_or("failed to get character")?;

            let modal = CreateModal::new("ai-charactereditor-prompt-modal", "Edit prompt")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Paragraph, "Prompt", "prompt")
                        .placeholder("\"A sentient, smart and very eager catgirl, etc, etc, etc\"")
                        .required(true)
                        .value(character.prompt.clone())),
                ]);

            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-charactereditor-description" => {
            let character_editor = data.character_editors.get(interaction.message.id).await?;
            let character = user_data.characters.iter().find(|&c| c.id == character_editor.character).ok_or("failed to get character")?;

            let modal = CreateModal::new("ai-charactereditor-description-modal", "Edit description")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Description", "description")
                        .placeholder("\"A custom character\"")
                        .required(false)
                        .value(character.description.clone())),
                ]);

            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-charactereditor-color" => {
            let character_editor = data.character_editors.get(interaction.message.id).await?;
            let character = user_data.characters.iter().find(|&c| c.id == character_editor.character).ok_or("failed to get character")?;

            let modal = CreateModal::new("ai-charactereditor-color-modal", "Edit color (change ONLY one)")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "RGB (0-255)", "rgb")
                        .required(true)
                        .value(
                            format!("{}, {}, {}", character.color.r(), character.color.g(), character.color.b())
                        )),
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "Hex color", "hex")
                        .required(true)
                        .value(format!("#{:06X}", character.color.0))),
                ]);

            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-charactereditor-avatar" => {
            let character_editor = data.character_editors.get(interaction.message.id).await?;
            let character = user_data.characters.iter().find(|&c| c.id == character_editor.character).ok_or("failed to get character")?;

            let modal = CreateModal::new("ai-charactereditor-avatar-modal", "Edit avatar")
                .components(vec![
                    CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short, "DIRECT url to the avatar", "pfp")
                        .placeholder("https://i.imgur.com/TLjpWLx.png")
                        .required(false)
                        .value(character.pfp_url.clone())),
                ]);

            interaction.create_response(&ctx, CreateInteractionResponse::Modal(modal)).await?;
        }
        "ai-charactereditor-delete" => {
            let mut character_editor = data.character_editors.get(interaction.message.id).await?;
            user_data.characters.retain(|c| c.id != character_editor.character);
            if user_data.characters.len() % 25 == 0 && character_editor.page > 0 {
                character_editor.page -= 1;
                user_data.character_editor.page -= 1;
            }
            data.user_database.set(user_id, &user_data).await?;

            character_editor.character = if !user_data.characters.is_empty() {
                &user_data.characters[0].id
            } else {
                ""
            }.to_string();
            data.character_editors.set(interaction.message.id, &character_editor).await?;

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = CreateInteractionResponseMessage::new()
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;

            send_system_followup(ctx, interaction, &user_data, "charactereditor-deleted").await?;
        }
        "ai-charactereditor-export" => {
            let character_editor = data.character_editors.get(interaction.message.id).await?;
            let character = user_data.characters.iter().find(|&c| c.id == character_editor.character).ok_or("failed to get character")?;

            let embed = create_character_embed(&user_data, "charactereditor-export")?;
            let character_json = serde_json::to_vec_pretty(character)?;

            let response = CreateInteractionResponseMessage::new()
                .add_embed(embed)
                .add_file(CreateAttachment::bytes(
                    character_json,
                    format!("{}.json", character.id)
                ))
                .ephemeral(true);

            interaction.create_response(&ctx, CreateInteractionResponse::Message(response)).await?;
        }
        "ai-charactereditor-prev" => {
            let mut character_editor = data.character_editors.get(interaction.message.id).await?;
            if character_editor.page != 0 {
                character_editor.page -= 1;
            } else {
                character_editor.page = 0;
            }
            data.character_editors.set(interaction.message.id, &character_editor).await?;

            user_data.character_editor.page = character_editor.page;
            data.user_database.set(user_id, &user_data).await?;

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = CreateInteractionResponseMessage::new()
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;
        }
        "ai-charactereditor-next" => {
            let mut character_editor = data.character_editors.get(interaction.message.id).await?;
            if character_editor.page < (user_data.characters.len() + 24) / 25 {
                character_editor.page += 1;
            } else {
                character_editor.page = 0;
            }
            data.character_editors.set(interaction.message.id, &character_editor).await?;

            user_data.character_editor.page = character_editor.page;
            data.user_database.set(user_id, &user_data).await?;
            
            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = CreateInteractionResponseMessage::new()
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;
        }
        "ai-charactereditor-page" => {
            interaction.create_response(&ctx, CreateInteractionResponse::Acknowledge).await?;
        }
        "ai-settings-prev" => {
            let mut settings = data.settings.get(interaction.message.id).await?;
            if settings.page != 0 {
                settings.page -= 1;
            } else {
                settings.page = 0;
            }
            data.settings.set(interaction.message.id, &settings).await?;

            user_data.settings.page = settings.page;
            data.user_database.set(user_id, &user_data).await?;

            let components = create_settings_components(&user_data, settings.page);
            let edit = CreateInteractionResponseMessage::new()
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;
        }
        "ai-settings-next" => {
            let mut settings = data.settings.get(interaction.message.id).await?;
            if settings.page < (user_data.characters.len() + 5 + 24) / 25 {
                settings.page += 1;
            } else {
                settings.page = 0;
            }
            data.settings.set(interaction.message.id, &settings).await?;
            
            user_data.settings.page = settings.page;
            data.user_database.set(user_id, &user_data).await?;

            let components = create_settings_components(&user_data, settings.page);
            let edit = CreateInteractionResponseMessage::new()
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;
        }
        "ai-settings-page" => {
            interaction.create_response(&ctx, CreateInteractionResponse::Acknowledge).await?;
        }
        _ => return Err("interaction id not found".into())
    }
    Ok(())
}

async fn modal_interaction(ctx: &Context, interaction: &ModalInteraction, data: &Data) -> Result<(), Error> {
    let user_id = interaction.user.id;
    let mut user_data = data.user_database.get(user_id).await?;

    let inputs: Vec<String> = interaction.data.components
        .iter().filter_map(|row| match row.components.first() {
            Some(ActionRowComponent::InputText(text)) => {
                text.value.clone()
            },
            Some(_) => None,
            None => None,
        })
        .collect();

    match interaction.data.custom_id.as_str() {
        "ai-settings-userdescription-modal" => {
            user_data.user_description.clone_from(&inputs[0]);
            data.user_database.set(user_id, &user_data).await?;
            
            let settings = data.settings.get(interaction.message.as_ref().unwrap().id).await?;

            let embed = create_character_embed(&user_data, "settings")?;
            let components = create_settings_components(&user_data, settings.page);

            let edit = CreateInteractionResponseMessage::new()
                .embed(embed)
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;

            send_system_followup(ctx, interaction, &user_data, "settings-userdescription-submitted").await?;
        }
        "ai-settings-scenario-modal" => {
            user_data.scenario.clone_from(&inputs[0]);
            data.user_database.set(user_id, &user_data).await?;
            
            let settings = data.settings.get(interaction.message.as_ref().unwrap().id).await?;

            let embed = create_character_embed(&user_data, "settings")?;
            let components = create_settings_components(&user_data,settings.page);

            let edit = CreateInteractionResponseMessage::new()
                .embed(embed)
                .components(components);

            interaction.create_response(&ctx, CreateInteractionResponse::UpdateMessage(edit)).await?;

            send_system_followup(ctx, interaction, &user_data, "settings-scenario-submitted").await?;
        }
        "ai-charactereditor-create-modal" => {
            let character = CharacterBuilder::default()
                .id(Uuid::new_v4().to_string())
                .description("A custom character".to_string())
                .prompt(inputs[2].clone())
                .name(inputs[0].clone())
                .display_name(if inputs[1].is_empty() { None } else { Some(inputs[1].clone()) })
                .attach_jb(true)
                .pfp_url("".to_string())
                .color(Color::LIGHT_GREY)
                .display_emote(None)
                .build()?;
            user_data.characters.push(character.clone());
            user_data.character_editor.character.clone_from(&character.id);
            data.user_database.set(user_id, &user_data).await?;

            let message_id = interaction.message.as_ref().unwrap().id;
            let mut character_editor = data.character_editors.get(message_id).await?;
            character_editor.character.clone_from(&character.id);

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = EditInteractionResponse::new()
                .components(components);
            edit.execute(ctx, &character_editor.interaction_token).await?;

            data.character_editors.set(message_id, &character_editor).await?;

            send_system_reply(ctx, interaction, &user_data, "charactereditor-created").await?;
        }
        "ai-charactereditor-name-modal" => {
            let character_editor = data.character_editors.get(interaction.message.as_ref().unwrap().id).await?;

            if let Some(character) = user_data.characters.iter_mut().find(|x| x.id == character_editor.character) {
                character.name.clone_from(&inputs[0]);
                character.display_name = if inputs[1].is_empty() { None } else { Some(inputs[1].clone()) };
            }
            data.user_database.set(user_id, &user_data).await?;

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = EditInteractionResponse::new()
                .components(components);
            edit.execute(ctx, &character_editor.interaction_token).await?;

            send_system_reply(ctx, interaction, &user_data, "charactereditor-edit").await?;
        }
        "ai-charactereditor-prompt-modal" => {
            let character_editor = data.character_editors.get(interaction.message.as_ref().unwrap().id).await?;

            if let Some(character) = user_data.characters.iter_mut().find(|x| x.id == character_editor.character) {
                character.prompt.clone_from(&inputs[0]);
            }
            data.user_database.set(user_id, &user_data).await?;

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = EditInteractionResponse::new()
                .components(components);
            edit.execute(ctx, &character_editor.interaction_token).await?;

            send_system_reply(ctx, interaction, &user_data, "charactereditor-edit").await?;
        }
        "ai-charactereditor-description-modal" => {
            let character_editor = data.character_editors.get(interaction.message.as_ref().unwrap().id).await?;

            if let Some(character) = user_data.characters.iter_mut().find(|x| x.id == character_editor.character) {
                character.description.clone_from(&inputs[0]);
            }
            data.user_database.set(user_id, &user_data).await?;

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = EditInteractionResponse::new()
                .components(components);
            edit.execute(ctx, &character_editor.interaction_token).await?;

            send_system_reply(ctx, interaction, &user_data, "charactereditor-edit").await?;
        }
        "ai-charactereditor-color-modal" => {
            let character_editor = data.character_editors.get(interaction.message.as_ref().unwrap().id).await?;

            if let Some(character) = user_data.characters.iter_mut().find(|x| x.id == character_editor.character) {
                let character_color = character.color.tuple();

                let rgb_color: (u8, u8, u8) = match inputs[0]
                    .split(',')
                    .map(|s| s.trim().parse().unwrap())
                    .collect::<Vec<u8>>()[..]
                {
                    [r, g, b] => (r, g, b),
                    _ => return Err("bad rgb format".into()),
                };

                let hex_color: (u8, u8, u8) = {
                    let input = inputs[1].trim_start_matches('#');
                    let hex_bytes = hex::decode(input).unwrap();
                    (hex_bytes[0], hex_bytes[1], hex_bytes[2])
                };

                if character_color != rgb_color && character_color != hex_color {
                    return Ok(()); // silently die
                }
                else if character_color != rgb_color {
                    character.color = Color::from(rgb_color);
                }
                else if character_color != hex_color {
                    character.color = Color::from(hex_color);
                }
            }
            data.user_database.set(user_id, &user_data).await?;

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = EditInteractionResponse::new()
                .components(components);
            edit.execute(ctx, &character_editor.interaction_token).await?;

            send_system_reply(ctx, interaction, &user_data, "charactereditor-edit").await?;
        }
        "ai-charactereditor-avatar-modal" => {
            let character_editor = data.character_editors.get(interaction.message.as_ref().unwrap().id).await?;

            if let Some(character) = user_data.characters.iter_mut().find(|x| x.id == character_editor.character) {
                character.pfp_url.clone_from(&inputs[0]);
            }
            data.user_database.set(user_id, &user_data).await?;

            let components = create_character_editor_components(&user_data, &character_editor.character, character_editor.page);
            let edit = EditInteractionResponse::new()
                .components(components);
            edit.execute(ctx, &character_editor.interaction_token).await?;

            send_system_reply(ctx, interaction, &user_data, "charactereditor-edit").await?;
        }
        _ => return Err("interaction id not found".into())
    }
    Ok(())
}

async fn command_interaction(ctx: &Context, interaction: &CommandInteraction, data: &Data) -> Result<(), Error> {
    let user_id = interaction.user.id;
    let mut user_data = data.user_database.get(user_id).await?;

    let inputs = &interaction.data.options;

    match interaction.data.name.as_str() {
        "uwu_reset" => {
            user_data.current_conversation.messages = [].to_vec();
            data.user_database.set(user_id, &user_data).await?;

            send_system_reply(ctx, interaction, &user_data, "reset").await?;
        }
        "uwu_settings" => {
            send_settings(ctx, interaction, user_data, data).await?;
        }
        "image" => {
            let character = get_character(&user_data.current_conversation.character, &user_data).ok_or("failed to get character")?;
            let splash_index = rand::thread_rng().gen_range(0..SPLASHES.len());
            let splash = SPLASHES[splash_index].to_string();
            
            let embed = CreateEmbed::new()
                .author(CreateEmbedAuthor::new(character.name.clone()).icon_url(character.pfp_url.clone()))
                .color(character.color)
                .description("Waiting in queue. (0/5)")
                .footer(CreateEmbedFooter::new(format!("all images are generated on my personal computer be patient • {}", splash)));
            
            let response = CreateInteractionResponseMessage::new()
                .embed(embed.clone());
            
            interaction.create_response(&ctx, CreateInteractionResponse::Message(response)).await?;

            let positive_prompt = inputs.iter()
                .find(|x| x.name == "prompt")
                .ok_or("could not find prompt")?
                .value
                .as_str()
                .ok_or("prompt was not str")?
                .to_string();
            let negative_prompt = inputs.iter()
                .find(|x| x.name == "negative_prompt")
                .and_then(|x| x.value.as_str())
                .map(|x| x.to_string());
            let nsfw = inputs.iter()
                .find(|x| x.name == "nsfw")
                .and_then(|x| x.value.as_bool());
            let cfg = inputs.iter()
                .find(|x| x.name == "cfg")
                .and_then(|x| x.value.as_f64())
                .map(|x| x as f32);
            let steps = inputs.iter()
                .find(|x| x.name == "steps")
                .and_then(|x| x.value.as_i64())
                .map(|x| x as u32);
            let sampler = inputs.iter()
                .find(|x| x.name == "sampler")
                .and_then(|x| x.value.as_str())
                .map(|x| x.to_string());
            let scheduler = inputs.iter()
                .find(|x| x.name == "scheduler")
                .and_then(|x| x.value.as_str())
                .map(|x| x.to_string());
            let (width, height) = inputs.iter()
                .find(|x| x.name == "aspect_ratio")
                .and_then(|x| x.value.as_str())
                .map(|x| x.split('x'))
                .map(|x| x.map(|v| v.parse::<u32>().unwrap()).collect::<Vec<u32>>())
                .map_or((None, None), |x| (Some(x[0]), Some(x[1])));
            
            let request = ImageRequestBuilder::default()
                .positive_prompt(positive_prompt)
                .negative_prompt(negative_prompt)
                .nsfw(nsfw)
                .cfg(cfg)
                .steps(steps)
                .sampler_name(sampler)
                .scheduler(scheduler)
                .width(width)
                .height(height)
                .build()
                .unwrap();
            
            info!("[{}] requested an image: {:#?}", interaction.user.name, request);

            let instant = Instant::now();
            
            let generate_image_result = generate_image(request).await;

            match generate_image_result {
                Ok(mut messages) => {
                    while let Some(message) = messages.next().await {
                        if let ImageGenerationMessage::Done(media_url) = message {
                            let embed = embed.clone()
                                .description(format!("Done in {:.2}s!\n[Download]({})", instant.elapsed().as_secs_f32(), media_url))
                                .image(media_url)
                                .footer(CreateEmbedFooter::new("images are saved for 30 days"));
                            interaction.edit_response(&ctx, EditInteractionResponse::new().embed(embed)).await?;
                            continue;
                        }
                        let description = match message {
                            ImageGenerationMessage::ExecutionStart => "Starting... (1/5)".to_string(),
                            ImageGenerationMessage::ModelLoad => "Loading model... (2/5)".to_string(),
                            ImageGenerationMessage::InferenceStart => "Starting generation... (3/5)".to_string(),
                            ImageGenerationMessage::InferenceStep(step) => format!("Generating at step {}/{} (4/5)", step, steps.unwrap_or(40)),
                            ImageGenerationMessage::Finishing => "Finishing... (5/5)".to_string(),
                            ImageGenerationMessage::Done(_) => { unreachable!() }
                            ImageGenerationMessage::Error(error) => format!("an error occured: {}\nif this keeps happening, contact support", error)
                        };
                        let embed = embed.clone().description(description);

                        interaction.edit_response(&ctx, EditInteractionResponse::new().embed(embed)).await?;
                    }
                }
                Err(error) => {
                    let embed = embed.clone().description(format!("could not establish a connection, the service is probably down.\nerror: {error}\nif this keeps happening in a few minutes, contact support"));

                    interaction.edit_response(&ctx, EditInteractionResponse::new().embed(embed)).await?;
                }
            }
        }
        "uwu_import" => {
            let attachment_id = inputs.first().ok_or("no file")?.value.as_attachment_id().ok_or("input was not attachement")?;
            let attachment = interaction.data.resolved.attachments.get(&attachment_id).ok_or("could not resolve attachment")?;
            let file = attachment.download().await?;
            let character: Character = serde_json::from_slice(&file)?;
            
            user_data.characters.retain(|c| c.id != character.id);
            user_data.characters.push(character);
            
            data.user_database.set(user_id, &user_data).await?;
            send_system_reply(ctx, interaction, &user_data, "charactereditor-import").await?;
        }
        _ => {}
    }
    
    Ok(())
}
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot { return; }

        let is_dm = if let Channel::Private(private_channel) = msg.channel(&ctx).await.unwrap() {
            matches!(private_channel.kind, ChannelType::Private)
        } else { false };

        if msg.mentions_me(&ctx).await.unwrap() || is_dm {
            info!("[{}] ai in [{}]", msg.author.name, msg.channel(&ctx).await.unwrap().to_string());

            let data = {
                let data = ctx.data.read().await;
                data.get::<GlobalData>().expect("failed to get data").clone()
            };
            let result = ai(&ctx, &data, &msg).await;
            match result {
                Ok(_) => {}
                Err(error) => {
                    error!("[{}] {}", msg.author.name, error);
                    let _ = msg.reply(&ctx, format!("an error occured: {}\nif this keeps happening, contact support", error)).await;
                }
            };
        };
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("connected as {}", ready.user.name);
        Command::set_global_commands(
            &ctx, 
            vec![
                CreateCommand::new("uwu_reset")
                    .description("Resets your conversation with uwu mew mew"),
                CreateCommand::new("uwu_settings")
                    .description("Shows settings of uwu mew mew"),
                CreateCommand::new("image")
                    .description("Generates an image using SDXL")
                    .add_option(CreateCommandOption::new(
                        CommandOptionType::String,
                        "prompt",
                        "The prompt to give to the model, ex. \"1girl, maid outfit\"").required(true))
                    .add_option(CreateCommandOption::new(
                        CommandOptionType::String,
                        "negative_prompt",
                        "An optional negative prompt. If none, uses the recommended default."))
                    .add_option(CreateCommandOption::new(
                        CommandOptionType::Boolean,
                        "nsfw",
                        "Steers the model towards sfw images if disabled."))
                    .add_option(CreateCommandOption::new(
                        CommandOptionType::Number,
                        "cfg",
                        "Specified how much the model should listen to the prompt. Higher numbers means more."))
                    .add_option(CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "steps",
                        "Number of steps that the image should be generated for. Maximum is 40.")
                        .min_int_value(0)
                        .max_int_value(40))
                    .add_option({
                        let mut option = CreateCommandOption::new(
                            CommandOptionType::String,
                            "sampler_name",
                            "Sampler that should be used.");
                        
                        for sampler in SAMPLERS {
                            option = option.add_string_choice(sampler, sampler);
                        }

                        option
                    })
                    .add_option({
                        let mut option = CreateCommandOption::new(
                            CommandOptionType::String,
                            "scheduler",
                            "Scheduler that should be used.");

                        for scheduler in SCHEDULERS {
                            option = option.add_string_choice(scheduler, scheduler);
                        }

                        option
                    })
                    .add_option(CreateCommandOption::new(
                        CommandOptionType::String,
                        "aspect_ratio",
                        "Aspect ratio that the image should be generated in.")
                        .add_string_choice("1:1 Square", "1024x1024")
                        .add_string_choice("9:7", "1152x896")
                        .add_string_choice("7:9", "896x1152")
                        .add_string_choice("19:13", "1216x832")
                        .add_string_choice("13:19", "832x1216")
                        .add_string_choice("7:4 Horizontal", "1344x768")
                        .add_string_choice("4:7 Vertical", "768x1344")
                        .add_string_choice("12:5 Horizontal", "1536x640")
                        .add_string_choice("5:12 Vertical", "640x1536")
                    ),
                CreateCommand::new("uwu_import")
                    .description("Imports a custom character file. Characters with the same id will be replaced.")
                    .add_option(CreateCommandOption::new(
                        CommandOptionType::Attachment,
                        "file",
                        "The .json file with the character data."
                    ).required(true))
            ]
        ).await.unwrap();
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let data = {
            let data = ctx.data.read().await;
            data.get::<GlobalData>().expect("failed to get data").clone()
        };

        let result = match interaction.kind() {
            InteractionType::Component => {
                let interaction = interaction.as_message_component().expect("wrong interaction type");
                info!("[{}] component interaction", interaction.user.name);
                component_interaction(&ctx, interaction, &data).await
            }
            InteractionType::Modal => {
                let interaction = interaction.as_modal_submit().expect("wrong interaction type");
                info!("[{}] modal interaction", interaction.user.name);
                modal_interaction(&ctx, interaction, &data).await
            }
            InteractionType::Command => {
                let interaction = interaction.as_command().expect("wrong interaction type");
                info!("[{}] command interaction", interaction.user.name);
                command_interaction(&ctx, interaction, &data).await
            }
            _ => { Err("not implemented".into()) }
        };

        handle_result(result, &ctx, interaction).await;
    }
}

async fn handle_result<T>(result: Result<(), Error>, ctx: &Context, interaction: T) where T : InteractionCreateResponse + InteractionCreateFollowup + InteractionUser {
    match result {
        Ok(_) => {}
        Err(error) => {
            error!("[{}] {}", interaction.user().name, error);
            let content = format!("an error occured: {}\nif this keeps happening, contact support", error);
            let result = interaction.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(content.clone())
                        .ephemeral(true)
                )
            ).await;

            if result.is_ok() {
                return;
            }

            let _ = interaction.create_followup(
                ctx,
                CreateInteractionResponseFollowup::new()
                    .content(content.clone())
                    .ephemeral(true)
            ).await;
        }
    };
}

struct GlobalData;

impl TypeMapKey for GlobalData {
    type Value = Arc<Data>;
}

#[tokio::main]
async fn main() {
    env_logger::init();
    
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents = GatewayIntents::non_privileged();

    let mut client = serenity::all::Client::builder(token, intents)
        .event_handler(Handler)
        .await
        .expect("could not create client");

    let gpt_openai_client = async_openai::Client::with_config(
        OpenAIConfig::new()
            .with_api_key(std::env::var("GPT_OPENAI_API_KEY").unwrap())
            .with_api_base(std::env::var("GPT_OPENAI_API_BASE").unwrap_or("https://api.openai.com/v1".to_string()))
    );
    let claude_openai_client = async_openai::Client::with_config(
        OpenAIConfig::new()
            .with_api_key(std::env::var("CLAUDE_OPENAI_API_KEY").unwrap())
            .with_api_base(std::env::var("CLAUDE_OPENAI_API_BASE").unwrap_or("https://api.openai.com/v1".to_string()))
    );

    let global_data = Data {
        gpt_openai_client,
        claude_openai_client,
        user_database: FsDatabase::create("user_data").await,
        character_editors: FsDatabase::create("character_editors").await,
        settings: FsDatabase::create("settings").await,
        ai_generations: FsDatabase::create("ai_generations").await,
    };

    {
        let mut data = client.data.write().await;

        data.insert::<GlobalData>(Arc::new(global_data));
    }

    client.start().await.expect("could not start client");
}