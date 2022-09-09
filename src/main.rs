mod konachan;
mod notify;
mod kde;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    notify::notify("Changing BG")?;
    let file_path = konachan::download(rand::random(), konachan::Filter::Explicit, true).await?;
    kde::set_wallpaper(&file_path).await?;
    Ok(())
}
