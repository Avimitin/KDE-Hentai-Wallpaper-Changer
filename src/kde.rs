use std::time::Duration;

use dbus::nonblock;

macro_rules! js {
    ($file:expr, $screen:expr) => {
        format!(
            r#"function set() {{
  var allDesktops = desktops();
  if (allDesktops.length < {}) return;
  var d = allDesktops[{}];
  d.wallpaperPlugin = "org.kde.image";
  d.currentConfigGroup = Array("Wallpaper",
                               "org.kde.image",
                               "General");
  d.writeConfig("Image", "{}");
}}

set();"#,
            $screen + 1,
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
    proxy
        .method_call(
            "org.kde.PlasmaShell",
            "evaluateScript",
            (js!(filename, screen),),
        )
        .await?;

    guardian.abort();

    Ok(())
}

#[test]
fn test_script_generation() {
    let script = js!("FUCK", 0);
    assert_eq!(
        script,
        r#"function set() {
  var allDesktops = desktops();
  if (allDesktops.length < 1) return;
  var d = allDesktops[0];
  d.wallpaperPlugin = "org.kde.image";
  d.currentConfigGroup = Array("Wallpaper",
                               "org.kde.image",
                               "General");
  d.writeConfig("Image", "FUCK");
}

set();"#
    )
}
