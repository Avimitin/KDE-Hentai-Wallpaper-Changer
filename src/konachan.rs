use std::{os::unix::prelude::FileExt, sync::Arc};

use anyhow::Context;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use md5::{Digest, Md5};
use serde::Deserialize;
use tokio::fs;
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

async fn ensure_temp_dir() -> anyhow::Result<String> {
    let mut temp_dir = std::env::temp_dir();
    temp_dir.push("konachan-wallpapers");
    let path = temp_dir.clone().to_str().unwrap().to_string();

    fs::create_dir_all(temp_dir).await?;

    Ok(path)
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

    parallel_download(&image_url, &save_to, arg).await?;

    Ok(save_to)
}

async fn get_image_filesize(client: &reqwest::Client, image_url: &str) -> anyhow::Result<u64> {
    let file_info = client
        .head(image_url)
        .header("user-agent", "Mozilla/5.0 (Android 4.4; Mobile; rv:41.0) Gecko/41.0 Firefox/41.0")
        .send()
        .await?;

    let filesize = file_info
        .headers()
        .get("content-length")
        .ok_or_else(|| anyhow::anyhow!("fail to retrieve image's filesize"))?;

    let error = "get invalid content-length header from url, please check your url correctness";
    let filesize = filesize
        .to_str()
        .unwrap_or_else(|_| panic!("{error}"))
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{error}"));

    Ok(filesize)
}

#[tokio::test]
async fn test_get_image_filesize() {
    let client = reqwest::Client::new();
    let url = "https://konachan.com/image/8007f7f332828bafa3a5877a2c5382d0/Konachan.com%20-%20347148%202girls%20ass%20barefoot%20bed%20black_hair%20breasts%20brown_hair%20cameltoe%20fingering%20long_hair%20nipples%20no_bra%20open_shirt%20panties%20pussy%20uncensored%20underwear%20yuri.png";

    let filesize = get_image_filesize(&client, url).await;
    assert_eq!(filesize.unwrap(), 4224138);
}

fn default_bar_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .expect("invalid template string")
        .progress_chars("#>-")
}

fn create_single_bar(
    m: &MultiProgress,
    length: u64,
    style: ProgressStyle,
    cur: usize,
    total: u8,
) -> ProgressBar {
    let bar = m.add(ProgressBar::new(length));
    bar.set_style(style);
    bar.set_prefix(format!("[{}/{}]", cur, total));
    bar
}

async fn download_partial(
    id: usize,
    client: &reqwest::Client,
    image_url: reqwest::Url,
    offset: (u64, u64),
    write_to: Arc<std::fs::File>,
    process_bar: Option<ProgressBar>,
) -> anyhow::Result<()> {
    let response = client
        .get(image_url)
        .header("Range", &format!("bytes={}-{}", offset.0, offset.1))
        .send()
        .await
        .with_context(|| format!("fail to download the {id} part of the image file"))?;

    let mut stream = response.bytes_stream();
    let mut progress_len = 0;
    let total_write = offset.1 - offset.0;
    let mut offset = offset.0;
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.with_context(|| format!("thread {id} fail to fetch chunk from upstream"))?;

        // WARN: This is not non-x86_64-linux compatible
        let writed = write_to
            .write_at(&chunk, offset)
            .with_context(|| format!("thread {id} fail to write content"))?;

        if let Some(ref bar) = process_bar {
            let writed_len = std::cmp::min(progress_len + (chunk.len() as u64), total_write);
            progress_len = writed_len;
            bar.set_position(progress_len);
        }

        offset += writed as u64;
    }

    Ok(())
}

async fn parallel_download(
    image_url: &str,
    save_to: &str,
    arg: &super::CliArg,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let filesize = get_image_filesize(&client, image_url).await?;

    // truncate file
    let file = Arc::new(std::fs::File::create(save_to)?);
    file.set_len(filesize)?;

    // calculate chunk size for each thread
    let threads = arg.download_threads as u64;
    let chunk_size = filesize / threads;

    let multi_progress = MultiProgress::new();
    let style = default_bar_style();

    let mut task = Vec::new();
    for i in 0..(arg.download_threads as usize) {
        let client = client.clone();
        let mut bar = None;
        if arg.show_process {
            let stylish_bar = create_single_bar(
                &multi_progress,
                chunk_size,
                style.clone(),
                i + 1,
                arg.download_threads,
            );

            bar = Some(stylish_bar)
        }

        let image_url = reqwest::Url::parse(image_url).unwrap();
        let file = Arc::clone(&file);
        let pad = i as u64;
        let start_offset = (chunk_size * pad) + pad;
        let end_offset = start_offset + chunk_size;

        let handle = tokio::spawn(async move {
            download_partial(i, &client, image_url, (start_offset, end_offset), file, bar).await
        });

        task.push(handle);
    }

    for t in task {
        t.await.unwrap()?;
    }

    multi_progress
        .clear()
        .expect("fail to clean the status bar");

    file.sync_all()?;

    Ok(())
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
