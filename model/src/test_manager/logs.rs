use super::{error, CrdName, Result, TestManager};
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::executor::block_on;
use futures::{SinkExt, Stream, StreamExt};
use k8s_openapi::{api::core::v1::Pod, chrono::NaiveDateTime};
use kube::{api::LogParams, Api, ResourceExt};
use regex::Regex;
use snafu::ResultExt;
use std::{cmp::Ordering, thread, time::Duration};

impl TestManager {
    /// Create a single ordered stream with logs from all `objects`.
    pub(super) async fn stream_logs(
        &self,
        objects: Vec<CrdName>,
        follow: bool,
    ) -> Result<impl Stream<Item = String>> {
        let pod_api: Api<Pod> = self.namespaced_api();
        let log_params = LogParams {
            follow,
            pretty: true,
            ..Default::default()
        };
        let mut logs = Vec::new();
        for crd in objects {
            for pod in self.get_pods(&crd).await? {
                logs.push(pod_api.log_stream(&pod.name(), &log_params).await.context(
                    error::KubeSnafu {
                        action: "stream logs",
                    },
                )?);
            }
        }
        struct Void {}

        let channels = logs
            .into_iter()
            .map(|logs| (logs, channel::<Void>(10), channel::<String>(10)))
            .map(|(mut logs, (tx, mut rx), (mut master_tx, master_rx))| {
                thread::spawn(move || {
                    while let Some(log) = block_on(logs.next()) {
                        let log_string = log
                            .map(|log| String::from_utf8_lossy(&log).to_string())
                            .unwrap_or_default();
                        let lines = log_string.lines();
                        for log_line in lines {
                            if block_on(master_tx.send(log_line.to_string())).is_err() {
                                return;
                            };
                            // The master thread will tell this thread when to retrieve the next value. This prevents channels
                            // from becoming clogged.
                            loop {
                                match rx.try_next() {
                                    Ok(None) => return,
                                    Ok(Some(_)) => break,
                                    Err(_) => std::thread::sleep(Duration::from_millis(50)),
                                }
                            }
                        }
                    }
                });
                (tx, master_rx)
            });
        let (mut stream_tx, stream_rx) = channel(100);
        thread::spawn(move || {
            fn replace_last(
                (mut tx, master_rx, last, finished): (
                    Sender<Void>,
                    Receiver<String>,
                    Option<String>,
                    bool,
                ),
                next: String,
            ) -> (Sender<Void>, Receiver<String>, Option<String>, bool) {
                // Alert the channel that we are ready for another log
                if last == Some(next) {
                    if let Err(e) = block_on(tx.send(Void {})) {
                        eprintln!("Some logs were lost: {}", e);
                    }
                    (tx, master_rx, Option::<String>::None, finished)
                } else {
                    (tx, master_rx, last, finished)
                }
            }
            std::thread::sleep(Duration::from_millis(50));
            // We will alter channels to store the last value sent by each channel
            #[allow(clippy::type_complexity)]
            let mut channels: Vec<(
                Sender<Void>,
                Receiver<String>,
                Option<String>,
                bool,
            )> = channels
                .map(|(tx, master_rx)| (tx, master_rx, None, false))
                .collect();
            loop {
                channels = channels
                    .into_iter()
                    .map(|(tx, mut master_rx, last, finished)| {
                        // If we havent used the last value
                        if last.is_some() || finished {
                            (tx, master_rx, last, finished)
                        } else {
                            match master_rx.try_next() {
                                Ok(None) => (tx, master_rx, last, true),
                                Ok(Some(next)) => (tx, master_rx, Some(next), false),
                                Err(_) => (tx, master_rx, last, finished),
                            }
                        }
                    })
                    .collect();
                if let Some(Some(next)) =
                    channels.iter().map(|(_, _, last, _)| last).min_by(|a, b| {
                        if let Some(a) = a {
                            if let Some(a) = log_timestamp(a) {
                                if let Some(b) = b {
                                    if let Some(b) = log_timestamp(b) {
                                        return a.cmp(&b);
                                    }
                                    return Ordering::Greater;
                                }
                                return Ordering::Less;
                            }
                            return Ordering::Less;
                        }
                        Ordering::Greater
                    })
                {
                    let next = next.to_string();
                    channels = channels
                        .into_iter()
                        .map(|channel| replace_last(channel, next.clone()))
                        .collect::<Vec<_>>();
                    if let Err(e) = block_on(stream_tx.send(next.to_string())) {
                        eprintln!("Some logs may have been lost: {}", e);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }

                if channels.iter().fold(true, |acc, (_, _, last, finished)| {
                    acc && *finished && last.is_none()
                }) {
                    return;
                }
            }
        });
        Ok(stream_rx)
    }
}

/// Extract the timestamp from an agents log.
fn log_timestamp(log: &str) -> Option<NaiveDateTime> {
    if let Ok(timestamp_regex) = Regex::new(r"^\[(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})Z.*$") {
        if let Some(time) = timestamp_regex
            .captures(log)
            .and_then(|captures| captures.get(1))
        {
            if let Ok(time) = NaiveDateTime::parse_from_str(time.as_str(), "%Y-%m-%dT%H:%M:%S") {
                return Some(time);
            }
        }
    }
    None
}
