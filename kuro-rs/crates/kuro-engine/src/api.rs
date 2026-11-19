use std::sync::Arc;

use tokio::net::TcpListener;

pub async fn try_bind(port: u16) -> Result<TcpListener, std::io::Error> {
    TcpListener::bind(("127.0.0.1", port)).await
}

pub async fn bind_with_retry(port: u16, log: &dyn Fn(&str)) -> TcpListener {
    loop {
        match try_bind(port).await {
            Ok(listener) => return listener,
            Err(err) => {
                log(&format!("api bind on {port} failed: {err}; retrying in 5 s"));
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

pub fn spawn_api_thread(state: Arc<kuro_ipc::AppState>, port: u16, log: impl Fn(&str) + Send + 'static) {
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
            log("api tokio runtime failed");
            return;
        };
        rt.block_on(async move {
            let listener = bind_with_retry(port, &log).await;
            kuro_ipc::serve(state, listener).await;
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn bind_conflict_keeps_ptt() {
        let port = {
            let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            listener.local_addr().unwrap().port()
        };
        let hold = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        let _ = hold;
        let start = Instant::now();
        assert!(try_bind(port).await.is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(hold);
        assert!(try_bind(port).await.is_ok());
    }
}
