use anyhow::Context;
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

async fn get_image(seed: u32, filter: Filter, hi_resolution: bool) -> anyhow::Result<String> {
    let page = seed % filter.get_limit(hi_resolution);
    let url = if hi_resolution {
        format!(
            "https://konachan.com/post.json?page={page}&tags=width%3A2560..+height%3A1600..+rating%3A{}",
            filter
        )
    } else {
        format!(
            "https://konachan.com/post.json?page={page}&tags=rating%3A{}",
            filter
        )
    };
    let mut resp = reqwest::get(&url).await?.json::<Vec<ApiResposne>>().await?;
    let choice = rand::random::<usize>() % 20;

    // swap_remove performs better than vec[idx].clone()
    Ok(resp.swap_remove(choice).file_url)
}

pub async fn download(seed: u32, filter: Filter, hi_resolution: bool) -> anyhow::Result<String> {
    let dir = ensure_temp_dir()
        .await
        .with_context(|| "fail to create temporary directory to store image files")?;

    let image_url = get_image(seed, filter, hi_resolution)
        .await
        .with_context(|| "fail to get image information")?;

    let url = reqwest::Url::parse(&image_url).unwrap();
    let filename = url.path_segments().unwrap().last().unwrap();
    let filepath = format!("{dir}/{filename}");

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
