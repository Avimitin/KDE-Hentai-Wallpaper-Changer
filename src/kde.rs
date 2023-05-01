use std::time::Duration;

use dbus::nonblock;

macro_rules! plasma_eval {
    ($file:expr, $screen:expr) => {
        format!(
            r#"
var desktop = desktopForScreen({});
desktop.wallpaperPlugin = "org.kde.image";
desktop.currentConfigGroup = Array("Wallpaper", "org.kde.image", "General");
desktop.writeConfig("Image", "{}");
"#,
            $screen,
            $file
        )
    };
}

pub async fn set_wallpaper(filename: &str, screen: u8) -> anyhow::Result<()> {
    let (resource, connection) = dbus_tokio::connection::new_session_sync()?;
    let guardian = tokio::spawn(async {
        resource.await;
    });

    let proxy = nonblock::Proxy::new(
        "org.kde.plasmashell",
        "/PlasmaShell",
        Duration::from_secs(2),
        connection,
    );

    // check screen id correctness
    let resp: (String,) = proxy
        .method_call(
            "org.kde.PlasmaShell",
            "evaluateScript",
            ("print(screenCount)",),
        )
        .await?;
    let max_screen: u8 = resp.0.parse()?;
    if max_screen <= screen {
        anyhow::bail!("screen id too large")
    }

    proxy
        .method_call(
            "org.kde.PlasmaShell",
            "evaluateScript",
            (plasma_eval!(filename, screen),),
        )
        .await?;

    guardian.abort();

    Ok(())
}
