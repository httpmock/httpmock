use std::collections::HashMap;

use reqwest::blocking::Client;
use tokio::sync::oneshot;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub async fn get_request(
    uri: &str,
    headers: Option<HashMap<String, String>>,
    proxy: Option<&str>,
) -> Result<(u16, String), BoxError> {
    let uri = uri.to_string();
    let proxy = proxy.map(|p| p.to_string());
    let headers = headers.unwrap_or_default();

    let (tx, rx) = oneshot::channel::<Result<(u16, String), BoxError>>();

    std::thread::spawn(move || {
        let mut builder = Client::builder()
            .danger_accept_invalid_certs(true) // for testing only
            .danger_accept_invalid_hostnames(true);

        if let Some(p) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(p).unwrap());
        }

        let client = match builder.build() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Err(Box::new(e)));
                return;
            }
        };

        // Build the GET request
        let mut req = client.get(&uri);
        for (k, v) in &headers {
            req = req.header(k, v);
        }

        // No default User-Agent → send only if explicitly passed

        let res = req.send();
        match res {
            Ok(r) => {
                let status = r.status().as_u16();
                match r.text() {
                    Ok(body) => {
                        let _ = tx.send(Ok((status, body)));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(Box::new(e)));
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(Err(Box::new(e)));
            }
        }
    });

    rx.await.unwrap()
}
