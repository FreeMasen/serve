
use warp::{filters::fs::dir};


#[tokio::main]
async fn main() {
    let mut args = std::env::args();
    let _ = args.next();
    let root = if let Some(root) = args.next() {
        root
    } else {
        "./".to_string()
    };
    

    let server = warp::serve(dir(root.clone()));
    if let Ok((addr, f)) = server.try_bind_ephemeral(([127, 0, 0, 1], 0)) {
        println!("listening on http://127.0.0.1:{}", addr.port());
        f.await;
    } else {
        let server = warp::serve(dir(root));
        println!("listening on http://127.0.0.1:3456");
        server.run(([127,0,0,1], 3456)).await;
    }
}
