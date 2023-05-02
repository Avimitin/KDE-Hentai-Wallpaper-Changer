mod kde;
mod konachan;
mod notify;

use std::{
    os::unix::prelude::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Context;
use argh::FromArgs;

fn default_filter() -> String {
    "none".to_string()
}

fn default_screen() -> u8 {
    0
}

fn default_thread_number() -> u8 {
    4
}

#[derive(FromArgs)]
/// Get anime image from Konachan(https://konachan.com) and set as wallpaper.
pub struct CliArg {
    /// download image larger than 2560x1600 only
    #[argh(switch, short = 'H')]
    hi_resolution: bool,

    /// filter image types.
    /// Avaliable types: Explicit(R18+), Questionable(R16+), Safe(SFW), None(All types possible)
    #[argh(option, short = 'f', default = "default_filter()")]
    filter: String,

    /// show download process
    #[argh(switch, short = 'P')]
    show_process: bool,

    /// disable notification
    #[argh(switch, short = 'N')]
    disable_notification: bool,

    /// save the current wallpapers, use variable $KDE_WALLPAPER_SAVE_DIR to specify directories to
    /// save the image. Default save image to $HOME/Pictures/Anime/.
    #[argh(switch)]
    save: bool,

    /// specify monitor id, useful for multi-screen user
    #[argh(option, short = 's', default = "default_screen()")]
    screen_id: u8,

    /// specify the thread number for multi-threads download
    #[argh(option, default = "default_thread_number()")]
    download_threads: u8,
}

fn dl_dir() -> PathBuf {
    let mut tmp_dir = std::env::temp_dir();
    tmp_dir.push("konachan-wallpapers");
    tmp_dir
}

fn fifo_file() -> PathBuf {
    let mut dl_dir = dl_dir();
    dl_dir.push(".last_wallpaper");
    dl_dir
}

fn save_into_fifo(filename: &str) -> anyhow::Result<()> {
    std::fs::write(fifo_file(), filename)
        .with_context(|| "fail to write image path into fifo file")?;
    Ok(())
}

fn read_from_fifo() -> anyhow::Result<PathBuf> {
    let content = std::fs::read(fifo_file()).with_context(|| {
        format!(
            "fail to read fifo info from temp file: {}",
            fifo_file().display()
        )
    })?;
    let path = String::from_utf8(content).with_context(|| "invalid path string")?;
    let path = Path::new(&path);
    Ok(path.to_path_buf())
}

fn save_wallpaper() -> anyhow::Result<()> {
    // Save to $KDE_WALLPAPER_SAVE_DIR or $HOME/Pictures/Anime
    let save_to_dir = std::env::var("KDE_WALLPAPER_SAVE_DIR").unwrap_or_else(|_| {
        let home_dir =
            std::env::var("HOME").unwrap_or_else(|_| panic!("Couldn't found your home directory"));
        let mut home_dir = PathBuf::from(home_dir);
        home_dir.push("Pictures");
        home_dir.push("Anime");
        home_dir.to_str().unwrap().to_string()
    });

    let copy_from = read_from_fifo()?;
    let filename = copy_from
        .file_name()
        .unwrap_or_else(|| panic!("invalid image name"));

    let mut save_to_dir = PathBuf::from(save_to_dir);
    save_to_dir.push(filename);

    std::fs::copy(&copy_from, &save_to_dir).with_context(|| "fail to save image")?;

    println!("File save to {}", save_to_dir.display());
    Ok(())
}

fn try_gc<P: AsRef<Path>>(download_dir: P) -> anyhow::Result<()> {
    let files = std::fs::read_dir(download_dir)?;
    let garbage = files
        .into_iter()
        .filter_map(|f| {
            let entry = f.ok()?;

            let metadata = entry.metadata().ok()?;
            if metadata.is_dir() {
                return None;
            }

            let mtime = metadata.modified().ok()?;
            let now = SystemTime::now();
            let elapsed = now
                .duration_since(mtime)
                .expect("You have a picture from the future, check your system time!");

            if elapsed > Duration::from_secs(60 * 30) {
                Some((entry.path(), metadata.size()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let (file_count, total_gc_size) =
        garbage
            .iter()
            .fold((0, 0), |(file_count, total_gc_size), (img, size)| {
                if let Err(err) = std::fs::remove_file(img) {
                    eprintln!("fail to clean up file {img:?}: {err}");

                    (file_count, total_gc_size)
                } else {
                    (file_count + 1, total_gc_size + size)
                }
            });

    println!(
        "Removed {file_count} files, save {} MB space",
        total_gc_size / 1048576
    );
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg: CliArg = argh::from_env();

    if arg.save {
        save_wallpaper()?;
        return Ok(());
    }

    if !arg.disable_notification {
        notify::notify("Changing BG")?;
    }

    let file_path = konachan::download(&arg).await?;
    kde::set_wallpaper(&file_path, arg.screen_id).await?;

    println!("Background is set to {file_path}");
    save_into_fifo(&file_path)?;

    try_gc(dl_dir())?;

    Ok(())
}

#[test]
fn test_try_gc() {
    try_gc("/tmp/konachan-wallpapers").unwrap();
}
