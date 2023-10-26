use std::{thread, collections::HashMap};

use crossbeam::channel::{unbounded, Sender};
use reqwest::{Client, Url};
use serde::Serialize;

use crate::{config::Config, location::Location};

pub(crate) struct LogServer {
    enabled: bool,
    api_key: String,
    url: Url,
    client: Client,
}

impl LogServer {
    pub fn from_config(config: &Config) -> Self {
        LogServer {
            enabled: config.server_logging.enabled,
            api_key: config.server_logging.api_key.clone(),
            url: Url::parse(&config.server_logging.url).expect("Invalid logging URL."),
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct LogMessage {
    pub(crate) class: String,
    pub(crate) content: String,
}

#[derive(Serialize)]
pub(crate) struct Log {
    api_key: String,
    message: LogMessage,
}

#[derive(Serialize)]
pub(crate) struct LocationUpdateEvent {
    pub location: Location,
    pub key_id: u64
}

pub(crate) struct StartupEvent {
    /// api key id -> username
    pub api_keys: HashMap<u64, String>,
    /// web user id -> (username, email)
    pub web_users: HashMap<u64, (String, String)>
}

pub(crate) struct LocationGetEvent {
	pub key_id: u64,
    pub user_id: u64,

}

pub(crate) fn start_logger(server: LogServer) -> Sender<LogMessage> {
    let (s, r) = unbounded::<LogMessage>();
    thread::spawn(|| async move {
        loop {
            let message = r.recv().unwrap();
            send_log(&server, message).await;
        }
    });
    s
}

pub(crate) async fn send_log(server: &LogServer, message: LogMessage) {
    if !server.enabled {
        return;
    }
    let client = server.client.clone();
    let message = Log {
        api_key: server.api_key.clone(),
        message,
    };
    let _ = client
        .post(server.url.clone())
        .body(serde_json::to_string(&message).unwrap())
        .send()
        .await;
}
