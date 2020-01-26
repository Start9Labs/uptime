use failure::{Error, ResultExt};
use reqwest::Url;
use serde::de::{Deserialize, Deserializer};

#[derive(Clone, Debug, serde::Deserialize)]
struct BasicAuth {
    username: String,
    password: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Webhook {
    #[serde(deserialize_with = "deser_url")]
    url: Url,
    message_param: Option<String>,
    basic_auth: Option<BasicAuth>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Server {
    enabled: bool,
    name: String,
    tor_address: String,
    webhook: Webhook,
    interval: std::time::Duration,
}
fn deser_url<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Url, D::Error> {
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Config {
    servers: Vec<Server>,
}

fn hit_callback(server: &Server, client: &reqwest::blocking::Client, message: String) {
    let mut callback = server.webhook.url.clone();
    if let Some(message_param) = &server.webhook.message_param {
        callback
            .query_pairs_mut()
            .append_pair(&message_param, &message);
    }
    let mut req = client.post(callback.clone());
    if let Some(ba) = &server.webhook.basic_auth {
        req = req.basic_auth(&ba.username, ba.password.as_ref());
    }
    if let Some(e) = match req.send() {
        Ok(res) => {
            if res.status().is_success() {
                None
            } else {
                Some(
                    res.status()
                        .canonical_reason()
                        .unwrap_or("UNKNOWN STATUS CODE")
                        .to_owned(),
                )
            }
        }
        Err(e) => Some(format!("{}", e)),
    } {
        eprintln!("error hitting {}: {}", callback, e);
        match (|| {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open("./start9/notifications.log")?;
            file.write_all(
                format!(
                    "{}:ERROR:1:Error Sending Callback:{}",
                    std::time::SystemTime::UNIX_EPOCH
                        .elapsed()
                        .map(|a| a.as_secs())
                        .unwrap_or(0),
                    e.replace("\n", "\u{2026}")
                )
                .as_bytes(),
            )?;
            file.flush()?;

            Ok::<_, Error>(())
        })() {
            Ok(_) => (),
            Err(e) => eprintln!("error saving notification: {}", e),
        }
    }
}

fn main() -> Result<(), Error> {
    let config: Config = serde_yaml::from_reader(
        std::fs::File::open("./start9/config.yaml")
            .with_context(|e| format!("./start9/config.yaml: {}", e))?,
    )?;
    let proxy = reqwest::Proxy::http(&format!(
        "socks5h://{}:9050",
        std::env::var("HOST_IP").with_context(|e| format!("HOST_IP: {}", e))?
    ))?;
    let client_base = reqwest::blocking::Client::builder().proxy(proxy).build()?;

    while config.servers.is_empty() {
        std::thread::sleep(std::time::Duration::from_secs(0x1000)); // a long-ass time
    }

    for server in config.servers.into_iter().filter(|c| c.enabled) {
        let client = client_base.clone();
        hit_callback(&server, &client, format!("{}: TEST", server.name));
        std::thread::spawn(move || loop {
            let req = client.get(&format!("http://{}:5959", server.tor_address));
            match req.send() {
                Ok(a) if a.status().is_success() => (),
                Ok(a) => hit_callback(
                    &server,
                    &client,
                    format!(
                        "{}: {}",
                        server.name,
                        a.status()
                            .canonical_reason()
                            .unwrap_or("UNKNOWN STATUS CODE")
                    ),
                ),
                Err(e) => hit_callback(&server, &client, format!("{}: {}", server.name, e)),
            }
            std::thread::sleep(server.interval);
        });
    }

    Ok(())
}
