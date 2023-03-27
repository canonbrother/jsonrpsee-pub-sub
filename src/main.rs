use futures::{Stream, StreamExt};
use jsonrpsee::{
    core::{
        client::{Subscription, SubscriptionClientT},
    },
    rpc_params,
    server::ServerBuilder,
    ws_client::WsClientBuilder,
    RpcModule, PendingSubscriptionSink, SubscriptionMessage, TrySendError
};
use serde::Serialize;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;

#[tokio::main]
async fn main() {
    let server = ServerBuilder::default()
        .build("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();

    let mut module = RpcModule::new(());

    module
        .register_subscription("sub_ping", "notif_ping", "unsub_ping", |_, mut pending, _| async {
            let interval = interval(Duration::from_millis(50));
            let stream = IntervalStream::new(interval).map(move |_| &"ping from sub");
            pipe_from_stream_and_drop(pending, stream).await.map_err::<anyhow::Error, _>(Into::into);
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

pub async fn pipe_from_stream_and_drop<T: Serialize>(
	pending: PendingSubscriptionSink,
	mut stream: impl Stream<Item = T> + Unpin,
) -> Result<(), anyhow::Error> {
	let mut sink = pending.accept().await?;

	loop {
		tokio::select! {
			_ = sink.closed() => return Err(anyhow::anyhow!("Subscription was closed")),
			maybe_item = stream.next() => {
				let item = match maybe_item {
					Some(item) => item,
					None => return Err(anyhow::anyhow!("Subscription executed successful")),
				};
				let msg = SubscriptionMessage::from_json(&item)?;

				match sink.try_send(msg) {
					Ok(_) => (),
					Err(TrySendError::Closed(_)) => return Err(anyhow::anyhow!("Subscription executed successful")),
					// channel is full, let's be naive an just drop the message.
					Err(TrySendError::Full(_)) => (),
				}
			}
		}
	}
}