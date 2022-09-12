# KDE Hentai Wallpaper Changer

A wallpaper changer for those weebs who use KDE.

<img src="./asserts/screenshot.png" width=45% />

## Usage

```bash
git clone https://github.com/Avimitin/KDE-Hentai-Wallpaper-Changer

cargo build --release
cp ./target/release/kwc $HOME/.local/bin

PATH="$PATH:$HOME/.local/bin" kwc --help
```

## Automatically Change with Systemd

```bash
cp -r services/* $HOME/.config/systemd/user/
systemctl --user start change-wallpaper.timer

# Automatically trigger after boot
systemctl --user enable change-wallpaper.timer
```

Update the duration in change-wallpaper.timer file. Use `man systemd.timer` to see more options.

> 献出你的底裤！
