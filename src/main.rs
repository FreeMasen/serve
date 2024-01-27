use std::{
    convert::Infallible,
    path::{Path, PathBuf},
};

use tokio::io::AsyncWriteExt;
use warp::{filters::fs::dir, log, reply::Html, Filter};

const INDEX_PREFIX: &str = "<!DOCTYPE html><html><head></head><body><main><ul>";
const INDEX_SUFFIX: &str = "</ul></body></html>";

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let mut args = std::env::args();
    let _ = args.next();
    let root = if let Some(root) = args.next() {
        PathBuf::from(root)
    } else {
        PathBuf::from("./")
    };
    let index_path = if !root.join("index.html").exists() {
        let temp = tempfile::Builder::default().tempfile().unwrap();
        let (_l, r) = temp.into_parts();
        let path = r.to_path_buf();
        tokio::task::spawn(spawn_index_generator(
            root.clone().canonicalize().unwrap(),
            r,
        ));
        path
    } else {
        root.join("index.html")
    };
    let index = warp::get()
        .and(warp::path::end().or(warp::path("index.html")))
        .and_then(move |_| read_path(index_path.clone()));

    let server = warp::serve(dir(root.clone()).or(index).with(log("serve")));
    if let Ok((addr, f)) = server.try_bind_ephemeral(([127, 0, 0, 1], 0)) {
        println!("listening on http://127.0.0.1:{}", addr.port());
        f.await;
    } else {
        println!("listening on http://127.0.0.1:3456");
        warp::serve(dir(root).with(log("serve")))
            .run(([127, 0, 0, 1], 3456))
            .await;
    }
}

async fn read_path(path: PathBuf) -> Result<Html<String>, Infallible> {
    let text = tokio::fs::read_to_string(path.clone())
        .await
        .unwrap_or_else(|e| format!("{INDEX_PREFIX}<li><pre><code>{e}</code></pre></li>{INDEX_SUFFIX}"));
    Ok(warp::reply::html(text))
}

async fn spawn_index_generator(root: PathBuf, index_path: impl AsRef<Path>) {
    loop {
        if let Err(e) = do_index_gen(&root, index_path.as_ref()).await {
            eprintln!("Error in index generation: {e}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn do_index_gen(
    root: impl AsRef<Path>,
    index: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let list = generate_file_list(&root).await?;
    write_index_html(index, list.into_iter()).await?;
    Ok(())
}

async fn generate_file_list(
    path: impl AsRef<Path>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut rd = tokio::fs::read_dir(path.as_ref()).await?;
    let mut ret = Vec::new();
    while let Some(entry) = rd.next_entry().await? {
        let Ok(ft) = entry.file_type().await else {
            continue;
        };
        if ft.is_file() && !ft.is_symlink() {
            let pb = entry.path();
            let Ok(relative) = pb.strip_prefix(path.as_ref()) else {
                continue;
            };
            ret.push(format!("{}", relative.display()));
        }
    }
    Ok(ret)
}

async fn write_index_html(
    path: impl AsRef<Path>,
    files: impl Iterator<Item = String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut f = tokio::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path.as_ref())
        .await
        .map_err(|e| {
            eprintln!("Error opening {}", path.as_ref().display());
            e
        })?;
    f.write_all(INDEX_PREFIX.as_bytes())
        .await
        .ok();
    for href in files {
        f.write_all(format!(r#"<li><a href="/{href}">{href}</a></li>"#).as_bytes())
            .await?;
    }
    f.write_all(INDEX_SUFFIX.as_bytes()).await?;
    Ok(())
}
