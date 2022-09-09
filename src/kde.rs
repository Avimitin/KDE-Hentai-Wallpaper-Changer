use std::time::Duration;

use dbus::nonblock;

macro_rules! js {
    ($file:expr) => {
        format!(r#"function set() {{
  var allDesktops = desktops();
  // if (allDesktops.length < 1) return;
  var d = allDesktops[0];
  d.wallpaperPlugin = "org.kde.image";
  d.currentConfigGroup = Array("Wallpaper",
                               "org.kde.image",
                               "General");
  d.writeConfig("Image", "{}");
}}

set();"#, $file)
    };
}

pub async fn set_wallpaper(filename: &str) -> anyhow::Result<()> {
    let (resource, connection) = dbus_tokio::connection::new_session_sync()?;
    let guardian = tokio::spawn(async {
        resource.await;
    });

    let proxy = nonblock::Proxy::new("org.kde.plasmashell", "/PlasmaShell", Duration::from_secs(2), connection);
    proxy.method_call("org.kde.PlasmaShell", "evaluateScript", (js!(filename), )).await?;

    guardian.abort();

    Ok(())
}

#[test]
fn test_script_generation() {
    let script = js!("FUCK");
    assert_eq!(script, r#"function set() {
  var allDesktops = desktops();
  // if (allDesktops.length < 1) return;
  var d = allDesktops[0];
  d.wallpaperPlugin = "org.kde.image";
  d.currentConfigGroup = Array("Wallpaper",
                               "org.kde.image",
                               "General");
  d.writeConfig("Image", "FUCK");
}

set();"#)
}

#[tokio::test]
async fn test_eval() {
    set_wallpaper("/tmp/wallpaper-download-files/Konachan.com%20-%20313298%202girls%20blue_hair%20blush%20braids%20close%20cropped%20headdress%20honkai_impact%20horns%20kiana_kaslana%20kiss%20long_hair%20raiden_mei%20shoujo_ai%20tears%20white%20wu_ganlan_cai.jpg").await.unwrap();
}
