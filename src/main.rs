mod kde;
mod konachan;
mod notify;

use argh::FromArgs;

fn default_filter() -> String {
    "none".to_string()
}

#[derive(FromArgs)]
/// Get anime image from Konachan(https://konachan.com) and set as wallpaper.
struct CliArg {
    /// download image larger than 2560x1600 only
    #[argh(switch, short = 'H')]
    hi_resolution: bool,

    /// filter image types.
    /// Avaliable types: Explicit(R18+), Questionable(R16+), Safe(SFW), None(All types possible)
    #[argh(option, short = 'f', default = "default_filter()")]
    filter: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg: CliArg = argh::from_env();
    notify::notify("Changing BG")?;
    let file_path = konachan::download(
        rand::random(),
        konachan::Filter::from(arg.filter),
        arg.hi_resolution,
    )
    .await?;
    kde::set_wallpaper(&file_path).await?;
    println!("Background is set to {file_path}");
    Ok(())
}
