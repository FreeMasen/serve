
use warp::{filters::fs::dir};

fn main() {
    let mut args = std::env::args();
    let _ = args.next();
    let root = if let Some(root) = args.next() {
        root
    } else {
        "./".to_string()
    };
    println!("listening on http://127.0.0.1:3456");
    warp::serve(dir(root)).run(([127,0,0,1], 3456));
}
