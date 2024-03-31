use async_openai::types::{ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent, Role};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserData {
    #[serde(default)]
    pub current_conversation: Conversation,
    #[serde(default)]
    pub characters: Vec<crate::characters::Character>,
    #[serde(default)]
    pub user_description: String,
    #[serde(default)]
    pub scenario: String,
    #[serde(default = "default_use_embed")]
    pub use_embed: bool,
    #[serde(default)]
    pub character_editor: CharacterEditorData,
    #[serde(default)]
    pub settings: SettingsData
}

impl Default for UserData {
    fn default() -> Self {
        Self {
            current_conversation: Conversation {
                messages: vec![],
                model: "gpt-4-0613".to_string(),
                character: "uwu-mew-mew-lite".to_string(),
            },
            characters: vec![],
            user_description: "".to_string(),
            scenario: "".to_string(),
            use_embed: true,
            character_editor: Default::default(),
            settings: Default::default(),
        }
    }
}

fn default_use_embed() -> bool { true }

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Conversation {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub character: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CharacterEditorData {
    pub character: String,
    pub page: usize
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SettingsData {
    pub page: usize
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChatMessage {
    pub content: ChatCompletionRequestUserMessageContent,
    pub role: Role,
    pub name: Option<String>,
    #[serde(default)]
    pub id: String
}

impl From<ChatMessage> for ChatCompletionRequestMessage {
    fn from(value: ChatMessage) -> Self {
        match value.role {
            Role::System => {
                if let ChatCompletionRequestUserMessageContent::Text(content) = value.content {
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessage {
                            content,
                            role: Role::System,
                            name: value.name,
                        }
                    )
                } else {
                    unreachable!();
                }
            }
            Role::User => {
                ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        content: value.content,
                        role: Role::User,
                        name: value.name,
                    }
                )
            }
            Role::Assistant => {
                if let ChatCompletionRequestUserMessageContent::Text(content) = value.content {
                    ChatCompletionRequestMessage::Assistant(
                        #[allow(deprecated)]
                        ChatCompletionRequestAssistantMessage {
                            content: content.into(),
                            role: Role::Assistant,
                            name: value.name,
                            tool_calls: None,
                            function_call: None,
                        }
                    )
                } else {
                    unreachable!();
                }
            }
            Role::Tool => { unimplemented!() }
            Role::Function => { unimplemented!() }
        }
    }
}

impl From<ChatCompletionRequestMessage> for ChatMessage {
    fn from(value: ChatCompletionRequestMessage) -> Self {
        match value {
            ChatCompletionRequestMessage::System(message) => {
                ChatMessage {
                    content: message.content.into(),
                    role: message.role,
                    name: message.name,
                    id: String::new(),
                }
            }
            ChatCompletionRequestMessage::User(message) => {
                ChatMessage {
                    content: message.content,
                    role: message.role,
                    name: message.name,
                    id: String::new(),
                }
            }
            ChatCompletionRequestMessage::Assistant(message) => {
                ChatMessage {
                    content: message.content.unwrap_or_else(|| unimplemented!()).into(),
                    role: message.role,
                    name: message.name,
                    id: String::new(),
                }
            }
            ChatCompletionRequestMessage::Tool(_) => { unimplemented!() }
            ChatCompletionRequestMessage::Function(_) => { unimplemented!() }
        }
    }
}