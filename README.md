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

- set filter_method for view -> #2324
- don't resize image on window resize (see comments in `view`) -> #2330
- perf improvements for image loading, especially on resizing -> #2334
- handle keybindings
- basic docs
- [repo] ci/cd
- [repo] dependabot

