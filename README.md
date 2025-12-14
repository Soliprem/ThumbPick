# ThumbPick

ThumbPick is a product of my procrastination. It's a lightweight,
keyboard-centric image picker written in Rust and GTK4. It scans a directory for
images, generates thumbnails asynchronously, and allows for rapid filtering and
selection.

It's designed to be used in scripts or pipelines. In particular, it does _one_
thing: it displays images, it allows you to search through them, and it returns
their path. And it does so quickly. All other tools I'd used before either
lacked one of these requirements, or also did a bunch more things I didn't need.

![screenshot](assets/screenshot.png)

## Features

- **Fast Thumbnail Generation**: Uses `rayon` for parallel image decoding and
  `walkdir` for efficient directory traversal.
- **Non-Blocking UI**: Image loading occurs on a separate thread, sending
  results to the main GTK loop via async channels to ensure the interface never
  freezes.
- **Type-to-Filter**: There is no search bar. Just start typing to filter images
  by filename. An overlay label appears dynamically at the bottom of the window
  to show the current query.
- **Scriptable Output**: Pressing `Enter` prints the selected image's full path
  to `stdout` and exits with status 0, making it easy to pipe into other tools.

## Usage

```bash
thumbpick <directory>
```

**Example:** Pipe the selected image to `feh` or a wallpaper setter:

```bash
# Set wallpaper with the selected image
swww img "$(thumbpick ~/Pictures/Wallpapers)"
```

## Controls

| Input            | Action                                   |
| ---------------- | ---------------------------------------- |
| **Alphanumeric** | Append character to search               |
| **Backspace**    | remove last character from filter (duh)  |
| **Escape**       | clear active filter                      |
| **Double Click** | Open image immediately with `xdg-open`   |
| **Enter**        | Print selected path to `stdout` and exit |

## Installation

### Nix (via flakes)

This project uses `naersk` and `fenix` to provide a reproducible build
environment.

**Run directly:**

```bash
nix run github:username/thumbpick <directory>
```

**Install:**

Add the flake to your inputs

```nix
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    thumbpick.url = "github:soliprem/thumbpick";
  ...
  };
```

And the package

```nix
environment.systemPackages = with pkgs; [
    ...
    inputs.thumbpick.packages.${pkgs.stdenv.hostPlatform.system}.default
    ...
];
```

Ready to go!

### Cargo

Ensure you have GTK4 development libraries installed on your system (should be
`gtk4`, `glib2`, `gdk-pixbuf2`, `cairo`, `pango`).

#### From Source

```bash
git clone https://github.com/soliprem/thumbpick
cd thumbpick
cargo install --path .
```

Or run directly without installing:

```bash
cargo run --release -- <directory>
```

## License

GNU General Public License v3.0.

## Credits

[waypaper](https://github.com/anufrievroman/waypaper) and
[waytrogen](https://github.com/nikolaizombie1/waytrogen) are massive design
inspirations. They're also just better projects if you also want the program to
handle setting the background automatically and don't want to script it
yourself.
