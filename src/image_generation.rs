use tokio::net::{TcpStream};

use derive_builder::Builder;
use futures::{Stream, stream};
use log::trace;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::Error;

#[derive(Serialize, Deserialize, Builder, Debug)]
pub struct ImageRequest {
    positive_prompt: String,
    #[builder(default)]
    negative_prompt: Option<String>,
    #[builder(default)]
    nsfw: Option<bool>,
    #[builder(default)]
    cfg: Option<f32>,
    #[builder(default)]
    steps: Option<u32>,
    #[builder(default)]
    width: Option<u32>,
    #[builder(default)]
    height: Option<u32>,
    #[builder(default)]
    seed: Option<u64>,
    #[builder(default)]
    sampler_name: Option<String>,
    #[builder(default)]
    scheduler: Option<String>,
}

pub enum ImageGenerationMessage {
    ExecutionStart,
    ModelLoad,
    InferenceStart,
    InferenceStep(u64),
    Finishing,
    Done(String),
    Error(String)
}
pub const SAMPLERS: [&str; 22] = [
    "euler",
    "euler_ancestral",
    "heun",
    "heunpp2",
    "dpm_2",
    "dpm_2_ancestral",
    "lms",
    "dpm_fast",
    "dpm_adaptive",
    "dpmpp_2s_ancestral",
    "dpmpp_sde",
    "dpmpp_sde_gpu",
    "dpmpp_2m",
    "dpmpp_2m_sde",
    "dpmpp_2m_sde_gpu",
    "dpmpp_3m_sde",
    "dpmpp_3m_sde_gpu",
    "ddpm",
    "lcm",
    "ddim",
    "uni_pc",
    "uni_pc_bh2",
];

pub const SCHEDULERS: [&str; 6] = [
    "normal",
    "karras",
    "exponential",
    "sgm_uniform",
    "simple",
    "ddim_uniform"
];

async fn read_message(stream: &mut TcpStream) -> Option<ImageGenerationMessage> {
    trace!("started reading message");
    let message = stream.read_u8().await.ok()?;
    trace!("message: {}", message);

    match message {
        1 => Some(ImageGenerationMessage::ExecutionStart),
        2 => Some(ImageGenerationMessage::ModelLoad),
        3 => Some(ImageGenerationMessage::InferenceStart),
        4 => Some(ImageGenerationMessage::InferenceStep(stream.read_u64_le().await.ok()?)),
        5 => Some(ImageGenerationMessage::Finishing),
        6 => {
            let len = stream.read_u64_le().await.ok()? as usize;
            let mut buf = vec![0u8; len];
            stream.read_exact(&mut buf).await.ok()?;

            let media_url = String::from_utf8(buf).ok()?;
            Some(ImageGenerationMessage::Done(media_url))
        }
        0xff => {
            let len = stream.read_u64_le().await.ok()? as usize;
            let mut error_bytes = vec![0u8; len];
            stream.read_exact(&mut error_bytes).await.ok()?;

            let error = String::from_utf8(error_bytes).ok()?;
            Some(ImageGenerationMessage::Error(error))
        }
        _ => unreachable!(),
    }
}

fn message_generator(stream: TcpStream) -> impl Stream<Item = ImageGenerationMessage> {
    trace!("started streaming messages");
    stream::unfold(stream, |mut stream| Box::pin(async move {
        let message = read_message(&mut stream).await;
        message.map(|m| (m, stream))
    }))
}
pub async fn generate_image(request: ImageRequest) -> Result<impl Stream<Item = ImageGenerationMessage>, Error> {
    trace!("started generating image");
    let mut stream = TcpStream::connect("127.0.0.1:8133").await?;
    trace!("established connection");
    
    let request_json = serde_json::to_vec(&request)?;
    stream.write_all(&request_json).await?;
    stream.shutdown().await?;
    trace!("wrote request");

    Ok(message_generator(stream))
}
