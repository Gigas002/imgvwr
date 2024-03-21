# imgvwr

Minimalistic, dead-simple image viewer with only function to view images, which follows unix philosophy

Powered by [iced](https://github.com/iced-rs/iced)

Features:

- lightweight
- native wayland support on linux
- crossplatform
- shows images

Cons:

- shows not all images (see `image` crate [features list](https://docs.rs/crate/image/latest/features))
- shows not moving images (e.g. `gif`)

Prefer using better software, e.g. [oculante](https://github.com/woelper/oculante) or [imv](https://sr.ht/~exec64/imv) if you want more features and proper support

## Usage

`imgvwr -i image.png`

## Config

Config file isn't created automatically. If you want to use your own config, copy repo's `config.toml` file into `~/.config/imgvwr/config.toml` and edit it. Or you can copy it elsewhere and pass via arguments: `imgvwr -i image.png -c config.toml`

See the `config.toml` file for possible values and explanations

## I want %feature%

I'd prefer to implement new features in upstream projects and provide a PR there in order to keep this project's code as simple as possible, so in *most* cases your issues are probably realted to something upstream. I also don't want to add any more external dependencies, at least for now, so keep it in mind. But feel free to ask anything in issues here too, I'm not against it in any way

As for common issues:

- if it's related to supported image formats, then you should check out the [image](https://github.com/image-rs/image) repo first. If the desired image format isn't supported in `image` - you can file a new issue there. Format's support in `iced` *may* be important too though

- if it's related to image-viewing capabilities, you should check out the [iced](https://github.com/iced-rs/iced) repo first, if your feature is implemented there. E.g. you want more `FilterMethod`s to be available. If there are such methods implemented in `iced` and not in `imgvwr` - file an issue/PR here and I'll add the support for it. If it's not - file an issue or PR into `iced` repo for this feature to be implemented there first

- if it's related to image modifications (paint, crop, etc) or a directory view then it's not planned

## TODO

- `FilterMethod` support for `Viewer` -> iced #2324
- `ContentFit` support for `Viewer` -> iced #2330
- rotate `widget` instead of `image` to improve perf/memory consuming -> iced #2334
- [repo] ci/cd
- [repo] dependabot
