use notify_rust::{ Notification, Timeout };

pub fn notify(text: &str) -> anyhow::Result<()> {
    Notification::new()
        .summary(text)
        .icon("photo")
        .appname("Wallpaper Changer")
        .timeout(Timeout::Milliseconds(6000)) //milliseconds
        .show()?;
    Ok(())
}

#[test]
fn test_notification() {
    notify("Changing BG").unwrap()
}
