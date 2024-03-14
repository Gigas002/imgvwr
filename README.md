# imgvwr

Minimalistic, dead-simple image viewer with only function to view images, which follows unix philosophy

Features:

- lightweight
- native support of wayland, thanks to `iced`
- shows images

Cons:

- shows not all images (see `image` crate supported format list)
- shows not moving images (`gif`s)

Prefer using better software, e.g. [oculante](https://github.com/woelper/oculante) or [imv](https://sr.ht/~exec64/imv) if you want more features and proper support

## Config

See `config.toml` for possible values and explanation

## TODO

- set filter_method for view (iced 0.13.0+)
- don't resize image on window resize (see comments in `view`)  -> working on draft pr for this
- handle keybindings
- check if strings can be separated from source to resources or something like that
- rename `ui` to `window` in `config`
