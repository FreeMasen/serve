
use warp::{filters::fs::dir, Filter, log};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let mut args = std::env::args();
    let _ = args.next();
    let root = if let Some(root) = args.next() {
        root
    } else {
        "./".to_string()
    };
    

    let server = warp::serve(dir(root.clone()).with(log("serve")));
    if let Ok((addr, f)) = server.try_bind_ephemeral(([127, 0, 0, 1], 0)) {
        println!("listening on http://127.0.0.1:{}", addr.port());
        f.await;
    } else {
        println!("listening on http://127.0.0.1:3456");
        warp::serve(dir(root).with(log("serve")))
            .run(([127,0,0,1], 3456)).await;
    }
}


