use anyhow::Context;
use md5::{Digest, Md5};
use serde::Deserialize;
use tokio::{fs, io::AsyncWriteExt};
use tokio_stream::StreamExt;

pub enum Filter {
    None,
    Safe,
    Explicit,
    Questionable,
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Filter::None => "",
            Filter::Safe => "safe",
            Filter::Explicit => "explicit",
            Filter::Questionable => "questionable",
        };
        write!(f, "{s}")
    }
}

impl From<String> for Filter {
    fn from(s: String) -> Self {
        let s = s.to_lowercase();
        match s.as_str() {
            "safe" => Self::Safe,
            "explicit" => Self::Explicit,
            "questionable" => Self::Questionable,
            "none" => Self::None,
            _ => panic!("Unsupported filter type: {s}")
        }
    }
}

impl Filter {
    fn get_limit(&self, hi_resolution: bool) -> u32 {
        use Filter::*;
        if hi_resolution {
            match self {
                None => 2000,
                Safe => 1200,
                Explicit => 300,
                Questionable => 650,
            }
        } else {
            match self {
                None => 13000,
                Safe => 2200,
                Questionable => 2800,
                Explicit => 1400,
            }
        }
    }
}

async fn ensure_temp_dir() -> anyhow::Result<&'static str> {
    let temp_dir = "/tmp/wallpaper-download-files";
    fs::create_dir_all(temp_dir).await?;
    Ok(temp_dir)
}

#[derive(Deserialize)]
struct ApiResposne {
    file_url: String,
}

fn gen_query(url: &mut reqwest::Url, page: u32, filter: Filter, hr: bool) {
    let mut handle = url.query_pairs_mut();
    if page != 0 {
        handle.append_pair("page", &page.to_string());
    }

    let mut tags = Vec::new();
    if hr {
        tags.push("width:2560..".to_string());
        tags.push("height:1600..".to_string());
    }

    if let Filter::None = filter {
        if !tags.is_empty() {
            handle.append_pair("tags", &tags[0]);
        }

        return;
    }

    tags.push(format!("rating:{filter}"));
    handle.append_pair("tags", &tags.join(" "));
}

async fn get_image(seed: u32, filter: Filter, hi_resolution: bool) -> anyhow::Result<String> {
    let page = seed % filter.get_limit(hi_resolution);
    let mut url = reqwest::Url::parse("https://konachan.com/post.json").unwrap();
    gen_query(&mut url, page, filter, hi_resolution);

    let mut resp = reqwest::get(url).await?.json::<Vec<ApiResposne>>().await?;
    let choice = rand::random::<usize>() % 20;

    // swap_remove performs better than vec[idx].clone()
    Ok(resp.swap_remove(choice).file_url)
}

fn md5sum(s: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(s.as_bytes());
    format!("{:?}", hasher.finalize().to_ascii_uppercase())
}

pub async fn download(seed: u32, filter: Filter, hi_resolution: bool) -> anyhow::Result<String> {
    let dir = ensure_temp_dir()
        .await
        .with_context(|| "fail to create temporary directory to store image files")?;

    let image_url = get_image(seed, filter, hi_resolution)
        .await
        .with_context(|| "fail to get image information")?;

    let url = reqwest::Url::parse(&image_url).unwrap();
    let filepath = format!("{dir}/{}", md5sum(url.as_str()));

    let mut file = fs::File::create(&filepath).await?;

    let mut stream = reqwest::get(url).await?.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }

    file.sync_all().await?;

    Ok(filepath)
}

#[tokio::test]
async fn test_download() {
    let path = download(rand::random(), Filter::Safe, true).await.unwrap();
    println!("{path}");
}
