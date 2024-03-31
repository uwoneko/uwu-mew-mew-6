#![allow(dead_code)]

use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart, ChatCompletionRequestUserMessageContent, Role};
use serenity::all::{CacheHttp, Channel, CommandInteraction, ComponentInteraction, CreateInteractionResponse, CreateInteractionResponseFollowup, CreateMessage, EditInteractionResponse, Http, Interaction, Message, MessageId, ModalInteraction, Result, User};
use serenity::Error;
use crate::user::ChatMessage;

pub trait InteractionCreateResponse {
    async fn create_response(
        &self,
        cache_http: impl CacheHttp,
        builder: CreateInteractionResponse,
    ) -> Result<()>;
}

impl InteractionCreateResponse for ComponentInteraction {
    async fn create_response(&self, cache_http: impl CacheHttp, builder: CreateInteractionResponse) -> Result<()> {
        self.create_response(cache_http, builder).await
    }
}

impl InteractionCreateResponse for ModalInteraction {
    async fn create_response(&self, cache_http: impl CacheHttp, builder: CreateInteractionResponse) -> Result<()> {
        self.create_response(cache_http, builder).await
    }
}

impl InteractionCreateResponse for CommandInteraction {
    async fn create_response(&self, cache_http: impl CacheHttp, builder: CreateInteractionResponse) -> Result<()> {
        self.create_response(cache_http, builder).await
    }
}

impl InteractionCreateResponse for Interaction {
    async fn create_response(&self, cache_http: impl CacheHttp, builder: CreateInteractionResponse) -> Result<()> {
        match self {
            Interaction::Component(interaction) => interaction.create_response(cache_http, builder).await,
            Interaction::Modal(interaction) => interaction.create_response(cache_http, builder).await,
            _ => unimplemented!()
        }
    }
}

pub trait InteractionCreateFollowup {
    async fn create_followup(
        &self,
        cache_http: impl CacheHttp,
        builder: CreateInteractionResponseFollowup,
    ) -> Result<Message>;
}

impl InteractionCreateFollowup for ComponentInteraction {
    async fn create_followup(&self, cache_http: impl CacheHttp, builder: CreateInteractionResponseFollowup) -> Result<Message> {
        self.create_followup(cache_http, builder).await
    }
}

impl InteractionCreateFollowup for ModalInteraction {
    async fn create_followup(&self, cache_http: impl CacheHttp, builder: CreateInteractionResponseFollowup) -> Result<Message> {
        self.create_followup(cache_http, builder).await
    }
}

impl InteractionCreateFollowup for Interaction {
    async fn create_followup(&self, cache_http: impl CacheHttp, builder: CreateInteractionResponseFollowup) -> Result<Message> {
        match self {
            Interaction::Component(interaction) => interaction.create_followup(cache_http, builder).await,
            Interaction::Modal(interaction) => interaction.create_followup(cache_http, builder).await,
            _ => unimplemented!()
        }
    }
}

pub trait InteractionDeleteResponse {
    async fn delete_response(
        &self,
        http: impl AsRef<Http>,
    ) -> Result<()>;
}

impl InteractionDeleteResponse for ComponentInteraction {
    async fn delete_response(&self, http: impl AsRef<Http>) -> Result<()> {
        self.delete_response(http).await
    }
}

impl InteractionDeleteResponse for ModalInteraction {
    async fn delete_response(&self, http: impl AsRef<Http>) -> Result<()> {
        self.delete_response(http).await
    }
}

impl InteractionDeleteResponse for CommandInteraction {
    async fn delete_response(&self, http: impl AsRef<Http>) -> Result<()> {
        self.delete_response(http).await
    }
}

impl InteractionDeleteResponse for Interaction {
    async fn delete_response(&self, http: impl AsRef<Http>) -> Result<()> {
        match self {
            Interaction::Component(interaction) => interaction.delete_response(http).await,
            Interaction::Modal(interaction) => interaction.delete_response(http).await,
            _ => unimplemented!()
        }
    }
}

pub trait InteractionDeleteFollowup {
    async fn delete_followup<M: Into<MessageId>>(
        &self,
        http: impl AsRef<Http>,
        message_id: M,
    ) -> Result<()>;
}

impl InteractionDeleteFollowup for ComponentInteraction {
    async fn delete_followup<M: Into<MessageId>>(&self, http: impl AsRef<Http>, message_id: M) -> Result<()> {
        self.delete_followup(http, message_id).await
    }
}

impl InteractionDeleteFollowup for ModalInteraction {
    async fn delete_followup<M: Into<MessageId>>(&self, http: impl AsRef<Http>, message_id: M) -> Result<()> {
        self.delete_followup(http, message_id).await
    }
}

pub trait InteractionGetResponse {
    async fn get_response(&self, http: impl AsRef<Http>) -> Result<Message>;
}

impl InteractionGetResponse for ComponentInteraction {
    async fn get_response(&self, http: impl AsRef<Http>) -> Result<Message> {
        self.get_response(http).await
    }
}

impl InteractionGetResponse for ModalInteraction {
    async fn get_response(&self, http: impl AsRef<Http>) -> Result<Message> {
        self.get_response(http).await
    }
}

impl InteractionGetResponse for CommandInteraction {
    async fn get_response(&self, http: impl AsRef<Http>) -> Result<Message> {
        self.get_response(http).await
    }
}
impl InteractionGetResponse for Interaction {
    async fn get_response(&self, http: impl AsRef<Http>) -> Result<Message> {
        match self {
            Interaction::Component(interaction) => interaction.get_response(http).await,
            Interaction::Modal(interaction) => interaction.get_response(http).await,
            _ => unimplemented!()
        }
    }
}

pub trait InteractionEditResponse {
    async fn edit_response(
        &self,
        cache_http: impl CacheHttp,
        builder: EditInteractionResponse,
    ) -> Result<Message>;
}

impl InteractionEditResponse for ComponentInteraction {
    async fn edit_response(&self, cache_http: impl CacheHttp, builder: EditInteractionResponse) -> Result<Message> {
        self.edit_response(cache_http, builder).await
    }
}

impl InteractionEditResponse for ModalInteraction {
    async fn edit_response(&self, cache_http: impl CacheHttp, builder: EditInteractionResponse) -> Result<Message> {
        self.edit_response(cache_http, builder).await
    }
}

impl InteractionEditResponse for Interaction {
    async fn edit_response(&self, cache_http: impl CacheHttp, builder: EditInteractionResponse) -> Result<Message> {
        match self {
            Interaction::Component(interaction) => interaction.edit_response(cache_http, builder).await,
            Interaction::Modal(interaction) => interaction.edit_response(cache_http, builder).await,
            _ => unimplemented!()
        }
    }
}

pub trait InteractionUser {
    fn user(&self) -> &User;
}

impl InteractionUser for Interaction {
    fn user(&self) -> &User {
        match self {
            Interaction::Component(interaction) => &interaction.user,
            Interaction::Modal(interaction) => &interaction.user,
            Interaction::Command(interaction) => &interaction.user,
            _ => unimplemented!()
        }
    }
}

impl InteractionUser for ComponentInteraction {
    fn user(&self) -> &User {
        &self.user
    }
}

impl InteractionUser for CommandInteraction {
    fn user(&self) -> &User {
        &self.user
    }
}

pub trait ChatMessageRole {
    fn role(&self) -> Role;
}

impl ChatMessageRole for ChatCompletionRequestMessage {
    fn role(&self) -> Role {
        match self {
            ChatCompletionRequestMessage::System(message) => message.role,
            ChatCompletionRequestMessage::User(message) => message.role,
            ChatCompletionRequestMessage::Assistant(message) => message.role,
            ChatCompletionRequestMessage::Tool(message) => message.role,
            ChatCompletionRequestMessage::Function(message) => message.role,
        }
    }
}

impl ChatMessageRole for ChatMessage {
    fn role(&self) -> Role {
        self.role
    }
}

pub trait ChatMessageContent {
    fn content(&self) -> String;
}

impl ChatMessageContent for ChatCompletionRequestMessage {
    fn content(&self) -> String {
        match self {
            ChatCompletionRequestMessage::System(message) => message.content.clone(),
            ChatCompletionRequestMessage::User(message) => { 
                match &message.content {
                    ChatCompletionRequestUserMessageContent::Text(content) => content.clone(),
                    ChatCompletionRequestUserMessageContent::Array(content) => {
                        if let ChatCompletionRequestMessageContentPart::Text(text) = content.first().unwrap() {
                            text.text.clone()
                        } else { 
                            unimplemented!();
                        }
                    }
                }
            },
            ChatCompletionRequestMessage::Assistant(message) => message.content.clone().unwrap(),
            ChatCompletionRequestMessage::Tool(message) => message.content.clone(),
            ChatCompletionRequestMessage::Function(message) => message.content.clone().unwrap(),
        }
    }
}

impl ChatMessageContent for ChatMessage {
    fn content(&self) -> String {
        match &self.content {
            ChatCompletionRequestUserMessageContent::Text(content) => content.clone(),
            ChatCompletionRequestUserMessageContent::Array(content) => {
                if let ChatCompletionRequestMessageContentPart::Text(text) = content.first().unwrap() {
                    text.text.clone()
                } else {
                    unimplemented!();
                }
            }
        }
    }
}

pub trait ChannelSendMessage {
    async fn send_message(
        &self,
        cache_http: impl CacheHttp,
        builder: CreateMessage,
    ) -> Result<Message>;
}

impl ChannelSendMessage for Channel {
    async fn send_message(&self, cache_http: impl CacheHttp, builder: CreateMessage) -> Result<Message> {
        match self {
            Channel::Guild(channel) => channel.send_message(cache_http, builder).await,
            Channel::Private(channel) => channel.send_message(cache_http, builder).await,
            _ => Err(Error::Other("unrecognized channel type")),
        }
    }
}