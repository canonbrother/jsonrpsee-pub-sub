// use anyhow::{Ok, Result};
use futures::{channel::mpsc, StreamExt};
use jsonrpsee::{
    core::{
        client::{Subscription, SubscriptionClientT},
        // error::{Error, SubscriptionClosed},
    },
    rpc_params,
    server::ServerBuilder,
    // types::SubscriptionEmptyError,
    ws_client::WsClientBuilder,
    RpcModule,
};

use std::net::SocketAddr;
use std::time::Duration;
// use tokio::sync::broadcast;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;

#[tokio::main]
async fn main() {
    let server = ServerBuilder::default()
        .build("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();

    let (tx, _rx) = mpsc::channel::<usize>(15);

    let mut module = RpcModule::new(tx);

    module
        .register_subscription("sub_ping", "notif_ping", "unsub_ping", |_, mut sink, _| {
            let interval = interval(Duration::from_millis(50));
            let stream = IntervalStream::new(interval).map(move |_| &"ping from sub");
            tokio::spawn(async move { sink.pipe_from_stream(stream).await });
            Ok(())
        })
        .unwrap();

    let addr = server.local_addr().unwrap();

    let handle = server.start(module).unwrap();

    tokio::spawn(handle.stopped());

    let url = format!("ws://{}", addr);

    let client = WsClientBuilder::default().build(&url).await.unwrap();

    let mut ping_sub: Subscription<String> = client
        .subscribe("sub_ping", rpc_params!(), "unsub_ping")
        .await
        .unwrap();

    for _ in 0..10 {
        let ping = ping_sub.next().await.unwrap().unwrap();
        println!("ping: {:?}", ping);
    }
}
