use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};
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

impl Filter {
    fn from_string(s: &str) -> anyhow::Result<Self> {
        let s = s.to_lowercase();
        let result = match s.as_str() {
            "safe" => Self::Safe,
            "explicit" => Self::Explicit,
            "questionable" => Self::Questionable,
            "none" => Self::None,
            _ => anyhow::bail!("Unsupported filter type: {s}"),
        };

        Ok(result)
    }

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
    let hash = hasher.finalize();
    base16ct::upper::encode_string(&hash)
}

fn escape_filename(url: &reqwest::Url) -> anyhow::Result<String> {
    let filename = url
        .path_segments()
        .ok_or_else(|| anyhow::anyhow!("fail to get filename from respond URL"))?
        .last()
        .ok_or_else(|| anyhow::anyhow!("not a file url"))?;
    let extension = std::path::Path::new(filename)
        .extension()
        .ok_or_else(|| anyhow::anyhow!("no extension found for this image"))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid string format for current OS"))?;

    Ok(format!("{}.{}", md5sum(url.as_str()), extension))
}

fn create_download_bar(filesize: u64) -> ProgressBar {
    let bar = ProgressBar::new(filesize);
    bar.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .expect("invalid template string")
        .progress_chars("#>-"));

    bar
}

pub async fn download(arg: &super::CliArg) -> anyhow::Result<String> {
    let dir = ensure_temp_dir()
        .await
        .with_context(|| "fail to create temporary directory to store image files")?;
    let filter = Filter::from_string(&arg.filter)?;

    let image_url = get_image(rand::random(), filter, arg.hi_resolution)
        .await
        .with_context(|| "fail to get image information")?;

    let url = reqwest::Url::parse(&image_url).unwrap();

    // konachan's file is incompatible for unix path, so here we use md5 to hash the url and use it
    // as filename.
    let save_to = format!("{dir}/{}", escape_filename(&url)?);

    let mut file = fs::File::create(&save_to).await?;

    let api_response = reqwest::get(url).await?;
    let filesize = api_response
        .content_length()
        .ok_or_else(|| anyhow::anyhow!("fail to get image size"))?;

    let mut stream = api_response.bytes_stream();

    let bar = create_download_bar(filesize);
    if arg.show_process {
        bar.set_message(format!("Downloading image to {save_to}"));
    }

    let mut progress_len = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        if arg.show_process {
            let writed_len = std::cmp::min(progress_len + (chunk.len() as u64), filesize);
            progress_len = writed_len;
            bar.set_position(progress_len);
        }
    }

    file.sync_all().await?;

    Ok(save_to)
}

#[tokio::test]
async fn test_download() {
    //let path = download(rand::random(), Filter::Safe, true, true).await.unwrap();
    //println!("{path}");
}

#[test]
fn test_md5sum() {
    let result = md5sum("fuckme");
    assert_eq!(result, "79CFDD0E92B120FAADD7EB253EB800D0");
}

#[test]
fn test_gen_query() {
    let mut url = reqwest::Url::parse("https://konachan.com/post.json").unwrap();

    let mut url1 = url.clone();
    gen_query(&mut url1, 0, Filter::None, false);
    // gen_query will append a question mark whatever we manipulated it or not. But it is
    // acceptable.
    assert_eq!(url1.as_str(), "https://konachan.com/post.json?");

    let mut url2 = url.clone();
    gen_query(&mut url2, 10, Filter::None, false);
    assert_eq!(url2.as_str(), "https://konachan.com/post.json?page=10");

    let mut url3 = url.clone();
    gen_query(&mut url3, 10, Filter::Safe, false);
    assert_eq!(
        url3.as_str(),
        "https://konachan.com/post.json?page=10&tags=rating%3Asafe"
    );

    gen_query(&mut url, 10, Filter::Safe, true);
    assert_eq!(
        url.as_str(),
        "https://konachan.com/post.json?page=10&tags=width%3A2560..+height%3A1600..+rating%3Asafe"
    );
}
