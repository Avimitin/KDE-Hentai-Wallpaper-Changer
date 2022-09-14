mod kde;
mod konachan;
mod notify;

use std::path::{Path, PathBuf};

use anyhow::Context;
use argh::FromArgs;

fn default_filter() -> String {
    "none".to_string()
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
}

fn temp_file() -> PathBuf {
    let mut tmp_dir = ::std::env::temp_dir();
    tmp_dir.push("konachan-wallpapers");
    tmp_dir.push(".last_wallpaper");
    tmp_dir
}

fn save_into_fifo(filename: &str) -> anyhow::Result<()> {
    std::fs::write(temp_file(), filename)
        .with_context(|| "fail to write image path into fifo file")?;
    Ok(())
}

fn read_from_fifo() -> anyhow::Result<PathBuf> {
    let content = std::fs::read(temp_file()).with_context(|| {
        format!(
            "fail to read fifo info from temp file: {}",
            temp_file().display()
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
    kde::set_wallpaper(&file_path).await?;

    println!("Background is set to {file_path}");
    save_into_fifo(&file_path)?;

    Ok(())
}
