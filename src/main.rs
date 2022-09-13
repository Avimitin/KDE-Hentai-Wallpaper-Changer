mod kde;
mod konachan;
mod notify;

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg: CliArg = argh::from_env();

    if !arg.disable_notification {
        notify::notify("Changing BG")?;
    }

    let file_path = konachan::download(&arg).await?;
    kde::set_wallpaper(&file_path).await?;

    println!("Background is set to {file_path}");
    Ok(())
}
