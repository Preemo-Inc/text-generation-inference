/// Copyright 2023 Michael Feil, text-generation-inference contributors
///
/// Licensed under the Apache License, Version 2.0 (the "License");
/// you may not use this file except in compliance with the License.
/// You may obtain a copy of the License at
///
///     http://www.apache.org/licenses/LICENSE-2.0
///
/// Unless required by applicable law or agreed to in writing, software
/// distributed under the License is distributed on an "AS IS" BASIS,
/// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
/// See the License for the specific language governing permissions and
/// limitations under the License.
///

/// Converting generate to completions and chat/completions protocol
use crate::{
    default_max_new_tokens, FinishReason, GenerateParameters, GenerateRequest, GenerateResponse,
    Info, OpenaiStreamType, StreamDetails, Token,
};
use axum::extract::Extension;
use axum::response::sse::Event;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub(crate) struct CompatCompletionRequest {
    #[schema(example = "My name is Michael and I")]
    pub prompt: String,
    #[serde(default)]
    #[schema(exclusive_minimum = 0, nullable = true, default = "null", example = 1)]
    pub best_of: Option<usize>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0.0,
        nullable = true,
        default = "null",
        example = 0.5
    )]
    pub temperature: Option<f32>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = -2.0,
        nullable = true,
        default = "null",
        example = 0.0
    )]
    pub presence_penalty: Option<f32>,
    // #[serde(default)]
    // #[schema(exclusive_minimum = 0, nullable = true, default = 1, example = 1)]
    // pub n: Option<i32>,
    #[serde(default)]
    #[schema(exclusive_minimum = 0, nullable = true, default = "null", example = 10)]
    pub top_k: Option<i32>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0.0,
        maximum = 1.0,
        nullable = true,
        default = "null",
        example = 0.95
    )]
    pub top_p: Option<f32>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0.0,
        maximum = 1.0,
        nullable = true,
        default = "null",
        example = 0.95
    )]
    pub typical_p: Option<f32>,
    #[serde(default)]
    #[schema(default = "false", example = true)]
    pub do_sample: bool,
    #[serde(default = "default_max_new_tokens")]
    #[schema(exclusive_minimum = 0, exclusive_maximum = 512, default = "20")]
    pub max_tokens: u32,
    #[serde(default)]
    #[schema(nullable = true, default = "null", example = false)]
    pub echo: Option<bool>,
    #[serde(default)]
    #[schema(inline, max_items = 4, example = json ! (["photographer"]))]
    pub stop: Vec<String>,
    #[serde(default)]
    #[schema(nullable = true, default = "null", example = "null")]
    pub truncate: Option<usize>,
    #[serde(default)]
    #[schema(default = "false", example = true)]
    pub watermark: bool,
    #[serde(default)]
    #[schema(default = "false")]
    pub decoder_input_details: bool,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0,
        nullable = true,
        default = "null",
        example = "null"
    )]
    pub seed: Option<u64>,
    #[serde(default)]
    #[schema(default = "false")]
    pub stream: bool,
}

impl From<CompatCompletionRequest> for GenerateRequest {
    fn from(req: CompatCompletionRequest) -> Self {
        let presence_penalty = req.presence_penalty;
        let presence_penalty = match presence_penalty {
            Some(presence_penalty) => Some((presence_penalty + 2.0) / 2.0),
            None => None,
        };
        Self {
            inputs: req.prompt,
            parameters: GenerateParameters {
                best_of: req.best_of,
                temperature: req.temperature,
                repetition_penalty: presence_penalty,
                top_k: req.top_k,
                top_p: req.top_p,
                typical_p: req.typical_p,
                do_sample: req.do_sample,
                max_new_tokens: req.max_tokens,
                return_full_text: req.echo,
                stop: req.stop,
                truncate: req.truncate,
                watermark: req.watermark,
                details: true,
                decoder_input_details: req.decoder_input_details,
                seed: req.seed,
            },
        }
    }
}

#[derive(Clone, Debug, ToSchema, Deserialize, Serialize)]
pub(crate) enum ChatRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct ChatFormatterPrePost {
    pre: String,
    post: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct ChatFormatter {
    user_template: ChatFormatterPrePost,
    assistant_template: ChatFormatterPrePost,
    system_template: ChatFormatterPrePost,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct ChatMessage {
    #[schema(example = "user")]
    role: ChatRole,
    #[schema(example = "What is the capital of Bavaria?")]
    content: String,
    // user: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub(crate) struct ChatDeltaStreamMessage {
    #[schema(example = "user")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<ChatRole>,
    #[schema(example = "What is the capital of Bavaria?")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    // user: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub(crate) struct CompatChatCompletionRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    #[schema(exclusive_minimum = 0, nullable = true, default = "null", example = 1)]
    pub best_of: Option<usize>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0.0,
        nullable = true,
        default = "null",
        example = 0.5
    )]
    pub temperature: Option<f32>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = -2.0,
        nullable = true,
        default = "null",
        example = 0.0
    )]
    pub presence_penalty: Option<f32>,
    // #[serde(default)]
    // #[schema(exclusive_minimum = 0, nullable = true, default = 1, example = 1)]
    // pub n: Option<u32>,
    #[serde(default)]
    #[schema(exclusive_minimum = 0, nullable = true, default = "null", example = 10)]
    pub top_k: Option<i32>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0.0,
        maximum = 1.0,
        nullable = true,
        default = "null",
        example = 0.95
    )]
    pub top_p: Option<f32>,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0.0,
        maximum = 1.0,
        nullable = true,
        default = "null",
        example = 0.95
    )]
    pub typical_p: Option<f32>,
    #[serde(default)]
    #[schema(default = "false", example = true)]
    pub do_sample: bool,
    #[serde(default = "default_max_new_tokens")]
    #[schema(exclusive_minimum = 0, exclusive_maximum = 512, default = "20")]
    pub max_tokens: u32,
    #[serde(default)]
    #[schema(nullable = true, default = "null", example = false)]
    pub echo: Option<bool>,
    #[serde(default)]
    #[schema(inline, max_items = 4, example = json ! (["photographer"]))]
    pub stop: Vec<String>,
    #[serde(default)]
    #[schema(nullable = true, default = "null", example = "null")]
    pub truncate: Option<usize>,
    #[serde(default)]
    #[schema(default = "false", example = true)]
    pub watermark: bool,
    #[serde(default)]
    #[schema(default = "false")]
    pub decoder_input_details: bool,
    #[serde(default)]
    #[schema(
        exclusive_minimum = 0,
        nullable = true,
        default = "null",
        example = "null"
    )]
    pub seed: Option<u64>,
    #[serde(default)]
    #[schema(default = "false")]
    pub stream: bool,
    // #[serde(default)]
    // #[schema(nullable = true, default = "null", example = "null")]
    // pub user: Option<String>,
}

pub(crate) fn chat_to_generate_request(
    req: CompatChatCompletionRequest,
    formatter: ChatFormatter,
) -> GenerateRequest {
    let mut prompt = String::from("");
    for m in req.messages {
        // let role = m.role
        let template = match m.role {
            ChatRole::Assistant => &formatter.assistant_template,
            ChatRole::System => &formatter.system_template,
            ChatRole::User => &formatter.user_template,
        };
        prompt.push_str(&template.pre);
        prompt.push_str(&m.content);
        prompt.push_str(&template.post);
    }
    let presence_penalty = req.presence_penalty;
    let presence_penalty = match presence_penalty {
        Some(presence_penalty) => Some((presence_penalty + 2.0) / 2.0),
        None => None,
    };

    GenerateRequest {
        inputs: prompt,
        parameters: GenerateParameters {
            best_of: req.best_of,
            temperature: req.temperature,
            repetition_penalty: presence_penalty,
            top_k: req.top_k,
            top_p: req.top_p,
            typical_p: req.typical_p,
            do_sample: req.do_sample,
            max_new_tokens: req.max_tokens,
            return_full_text: req.echo,
            stop: req.stop,
            truncate: req.truncate,
            watermark: req.watermark,
            details: true,
            decoder_input_details: req.decoder_input_details,
            seed: req.seed,
        },
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Usage {
    #[schema(example = 1)]
    pub total_tokens: u32,
    #[schema(example = 1)]
    pub completion_tokens: u32,
    #[schema(example = 1)]
    pub prompt_tokens: u32,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CompletionChoices {
    #[schema(example = "test")]
    pub text: String,
    #[schema(example = "length")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    // pub generated_tokens: u32,
    // logprobs is not implemented, send None
    pub logprobs: Option<Vec<u32>>,
    #[schema(example = 0)]
    pub index: u32,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CompletionsResponse {
    #[schema(example = "cmpl-abcdefgehij1234")]
    pub id: String,
    #[schema(example = "text_completion")]
    pub object: String,
    #[schema(example = 1589478379)]
    pub created: u64,
    #[schema(example = "tgi")]
    pub model: String,
    pub choices: Vec<CompletionChoices>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ChatCompletionChoices {
    #[schema(example = "test")]
    pub message: ChatMessage,
    #[schema(example = "length")]
    pub finish_reason: Option<FinishReason>,
    // pub generated_tokens: u32,
    #[schema(example = 0)]
    pub index: u32,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ChatCompletionDeltaStreamChoices {
    #[schema(example = "test")]
    pub delta: ChatDeltaStreamMessage,
    #[schema(example = "length")]
    pub finish_reason: Option<FinishReason>,
    // pub generated_tokens: u32,
    #[schema(example = 0)]
    pub index: u32,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ChatCompletionsResponse {
    #[schema(example = "chatcmpl-abcdefgehij1234")]
    pub id: String,
    #[schema(example = "chat.completion")]
    pub object: String,
    #[schema(example = 1589478380)]
    pub created: u64,
    #[schema(example = "tgi")]
    pub model: String,
    pub choices: Vec<ChatCompletionChoices>,
    pub usage: Usage,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ChatCompletionsStreamResponse {
    #[schema(example = "chatcmpl-abcdefgehij1234")]
    pub id: String,
    #[schema(example = "chat.completion.chunk")]
    pub object: String,
    #[schema(example = 1589478380)]
    pub created: u64,
    #[schema(example = "tgi")]
    pub model: String,
    pub choices: Vec<ChatCompletionDeltaStreamChoices>,
}

pub(crate) fn get_chatformatter() -> ChatFormatter {
    // TODO: improve reading this, e.g. at startup once from a chat_config.json
    let chat_user_pre: String = match std::env::var_os("TGICHAT_USER_PRE") {
        Some(v) => v.into_string().unwrap(),
        None => String::from(""),
    };
    let chat_user_post: String = match std::env::var_os("TGICHAT_USER_POST") {
        Some(v) => v.into_string().unwrap(),
        None => String::from(""),
    };
    let chat_ass_pre: String = match std::env::var_os("TGICHAT_ASS_PRE") {
        Some(v) => v.into_string().unwrap(),
        None => String::from(""),
    };
    let chat_ass_post: String = match std::env::var_os("TGICHAT_ASS_POST") {
        Some(v) => v.into_string().unwrap(),
        None => String::from(""),
    };
    let chat_sys_pre: String = match std::env::var_os("TGICHAT_SYS_PRE") {
        Some(v) => v.into_string().unwrap(),
        None => String::from(""),
    };
    let chat_sys_post: String = match std::env::var_os("TGICHAT_SYS_POST") {
        Some(v) => v.into_string().unwrap(),
        None => String::from(""),
    };

    ChatFormatter {
        user_template: ChatFormatterPrePost {
            pre: chat_user_pre,
            post: chat_user_post,
        },
        assistant_template: ChatFormatterPrePost {
            pre: chat_ass_pre,
            post: chat_ass_post,
        },
        system_template: ChatFormatterPrePost {
            pre: chat_sys_pre,
            post: chat_sys_post,
        },
    }
}

pub(crate) async fn generate_to_completions(
    resp: Json<GenerateResponse>,
    info: Extension<Info>,
) -> Json<CompletionsResponse> {
    // let details = resp.details.as_ref().ok_or("details missing"); //;
    let details = resp.details.as_ref();

    let gen_tokens = match details {
        Some(details) => details.generated_tokens,
        None => 0,
    };
    let finish_reason = match details {
        Some(details) => Some(details.finish_reason.clone()),
        None => None,
    };
    let prefill_len = match details {
        Some(details) => details.prefill.len() as u32,
        None => 0,
    };

    let choices = CompletionChoices {
        text: resp.generated_text.clone(),
        finish_reason: finish_reason,
        logprobs: None,
        index: 0,
    };
    let usage = Some(Usage {
        completion_tokens: gen_tokens,
        total_tokens: gen_tokens + prefill_len,
        prompt_tokens: prefill_len,
    });
    let created_time = create_timestamp();
    let model = info.0.model_id;
    let resp: CompletionsResponse = CompletionsResponse {
        choices: vec![choices],
        created: created_time,
        id: String::from(format!("cmpl-{}", created_time)),
        object: String::from("text_completion"),
        model,
        usage,
    };
    Json(resp.into())
}

pub(crate) async fn generate_to_chatcompletions(
    resp: Json<GenerateResponse>,
    info: Extension<Info>,
) -> Json<ChatCompletionsResponse> {
    // let details = resp.details.as_ref().ok_or("details missing"); //;
    let details = resp.details.as_ref();

    let gen_tokens = match details {
        Some(details) => details.generated_tokens,
        None => 0,
    };
    let finish_reason = match details {
        Some(details) => Some(details.finish_reason.clone()),
        None => None,
    };
    let prefill_len = match details {
        Some(details) => details.prefill.len() as u32,
        None => 0,
    };

    let choices = ChatCompletionChoices {
        message: ChatMessage {
            role: ChatRole::Assistant,
            content: resp.generated_text.clone(),
        },
        finish_reason: finish_reason,
        index: 0,
    };
    let usage = Usage {
        completion_tokens: gen_tokens,
        total_tokens: gen_tokens + prefill_len,
        prompt_tokens: prefill_len,
    };
    let created_time = create_timestamp();
    let model = info.0.model_id;
    let resp = ChatCompletionsResponse {
        choices: vec![choices],
        created: created_time,
        id: String::from(format!("chatcmpl-{}", created_time)),
        object: String::from("chat.completion"),
        model,
        usage,
    };
    Json(resp.into())
}

pub (crate) fn create_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs() as u64
}

pub(crate) fn chat_start_message(
    created_time: u64,
    model_name: &String,
) -> ChatCompletionsStreamResponse {
    let choices: ChatCompletionDeltaStreamChoices = ChatCompletionDeltaStreamChoices {
        delta: ChatDeltaStreamMessage {
            content: None,
            role: Some(ChatRole::Assistant),
        },
        finish_reason: None,
        index: 0,
    };
    ChatCompletionsStreamResponse {
        choices: vec![choices],
        created: created_time,
        id: String::from(format!("chatcmpl-{}", created_time)),
        object: String::from("chat.completion.chunk"),
        model: model_name.to_owned(),
    }
}

pub(crate) fn create_streaming_event(
    // st: StreamResponse,
    stream_type: &OpenaiStreamType,
    created_time: u64,
    details: Option<StreamDetails>,
    token: Token,
    model_name: &String,
) -> Event {
    match stream_type {
        &OpenaiStreamType::ChatCompletionsStreamResponse => {
            let choices: ChatCompletionDeltaStreamChoices = ChatCompletionDeltaStreamChoices {
                delta: ChatDeltaStreamMessage {
                    content: Some(token.text),
                    role: None,
                },
                finish_reason: match details {
                    Some(i) => Some(i.finish_reason),
                    None => None,
                },
                index: 0,
            };
            let response = ChatCompletionsStreamResponse {
                choices: vec![choices],
                created: created_time,
                id: String::from(format!("chatcmpl-{}", created_time)),
                object: String::from("chat.completion.chunk"),
                model: model_name.to_owned(),
            };
            Event::default().json_data(response).expect("cannot parse ChatCompletionsStreamResponse")
        }
        &OpenaiStreamType::CompletionsResponse => {
            let choices = CompletionChoices {
                text: token.text,
                finish_reason: match details {
                    Some(i) => Some(i.finish_reason),
                    None => None,
                },
                logprobs: None,
                index: 0,
            };

            let response = CompletionsResponse {
                choices: vec![choices],
                created: created_time,
                id: String::from(format!("cmpl-{}", created_time)),
                object: String::from("text_completion"),
                model: model_name.to_owned(),
                usage: None,
            };
            Event::default().json_data(response).expect("cannot parse streamed CompletionsResponse")
        }
    }
}
