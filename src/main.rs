use std::path::{Path, PathBuf};

use axum::{response::IntoResponse, Router};
use tokio::io::AsyncWriteExt;

const INDEX_PREFIX: &str = "<!DOCTYPE html><html><head></head><body><main><ul>";
const INDEX_SUFFIX: &str = "</ul></body></html>";

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let Args { root, prefix, port } = parse_args();

    let index_path: PathBuf = if !root.join("index.html").exists() {
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
    let index_cb = {
        let index_path = index_path.clone();
        let prefix = prefix.clone();
        let base_path = root.clone();
        move |uri: axum::http::Uri| async move {
            log::trace!("getting {uri}");
            let path = uri.path().trim_start_matches(&prefix);
            let path = if path == "" || path == "/" || path == "/index.html" || path == "index.htm"
            {
                index_path.clone()
            } else {
                base_path.join(path)
            };
            read_path(path).await
        }
    };

    let app = Router::new().fallback(index_cb);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let msg = format!("Listening on http://{}:{}", addr.ip(), addr.port());
    let f = tokio::task::spawn(async move { axum::serve(listener, app).await.unwrap() });
    log::info!("{msg}");
    f.await.unwrap();
}

struct Args {
    root: PathBuf,
    prefix: String,
    port: u16,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let root = if let Some(root) = args.next() {
        PathBuf::from(root)
    } else {
        PathBuf::from("./")
    };
    let mut ret = Args {
        root,
        prefix: "/".to_string(),
        port: 0,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--prefix" => ret.prefix = args.next().expect("--prefix requires value"),
            "--port" => {
                ret.port = args
                    .next()
                    .expect("--port requires value")
                    .parse()
                    .expect("invalid port")
            }
            a => panic!("Unknown argument {}", a),
        }
    }
    ret
}

async fn read_path(mut path: PathBuf) -> Response {
    if path.is_dir() {
        path = path.join("index.html")
    }
    log::debug!("reading {}", path.display());
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("html");
    let inner = match tokio::fs::read(path.clone()).await {
        Ok(inner) => inner,
        Err(e) => return Response::Html(format!("{INDEX_PREFIX}<li><pre><code>{e}</code></pre></li>{INDEX_SUFFIX}").as_bytes().to_vec())
    };
    match ext {
        "css" => Response::Css(inner),
        "js" => Response::Js(inner),
        "wasm" => Response::Wasm(inner),
        _ => Response::Html(inner),
    }
}

async fn spawn_index_generator(root: PathBuf, index_path: impl AsRef<Path>) {
    loop {
        if let Err(e) = do_index_gen(&root, index_path.as_ref()).await {
            eprintln!("Error in index generation: {e}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn do_index_gen(root: impl AsRef<Path>, index: impl AsRef<Path>) -> Result<(), Error> {
    if !root.as_ref().exists() {
        ::log::warn!("Root no longer exists at {}", root.as_ref().display());
        return Ok(());
    }
    let list = generate_file_list(&root).await.unwrap_or_default();
    write_index_html(index, list.into_iter()).await?;
    Ok(())
}

async fn generate_file_list(path: impl AsRef<Path>) -> Result<Vec<String>, Error> {
    let mut rd = tokio::fs::read_dir(path.as_ref())
        .await
        .map_err(|e| Error::ReadDir(format!("Error reading {}: {e}", path.as_ref().display())))?;
    let mut ret = Vec::new();
    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|e| Error::Entry(format!("Error looking up next entry: {e}")))?
    {
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
) -> Result<(), Error> {
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
    f.write_all(INDEX_PREFIX.as_bytes()).await.ok();
    for href in files {
        f.write_all(format!(r#"<li><a href="/{href}">{href}</a></li>"#).as_bytes())
            .await?;
    }
    f.write_all(INDEX_SUFFIX.as_bytes()).await?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    ReadDir(String),
    #[error("{0}")]
    Entry(String),
}

enum Response {
    Html(Vec<u8>),
    Css(Vec<u8>),
    Js(Vec<u8>),
    Wasm(Vec<u8>)
}

impl IntoResponse for Response {
    fn into_response(self) -> axum::response::Response {
        let res = axum::response::Response::builder();
        match self {
            Response::Html(vec) => {
                res.header("content-type", "text/html").body(vec.into()).unwrap()
            }
            Response::Css(vec) => {
                res.header("content-type", "text/css").body(vec.into()).unwrap()
            }
            Response::Js(vec) => {
                res.header("content-type", "application/javascript").body(vec.into()).unwrap()
            }
            Response::Wasm(vec) => {
                res.header("content-type", "application/wasm").body(vec.into()).unwrap()
            }
        }
    }
}
