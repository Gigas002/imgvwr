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
- shows not moving images (e.g. `gif`) -> iced #1412

Prefer using better software if you want more features and proper support

## Why then?

Because there's some stuff I don't like, of course. Personally I'd recommend to use these amazing tools:

- [imageglass](https://github.com/d2phap/ImageGlass) -- I remember it being pretty good, but since it's windows-only I haven't used it for last several years, so I don't know what's current state of the project
- [qimgv](https://github.com/easymodo/qimgv) -- I think it's the best tool for x11 session, but it has some minor issues on wayland session. These are partially resolved when compiling it with `qt6`, but it needs proper migration for this to work out. The main *cons* it that it's not developed actively for last two years. For pros, it has the best upscaling filter I've encountered in open source image viewers so far and handles a lot of formats. I also absolutely love how `gif`s playback implemented out here
- [imv](https://sr.ht/~exec64/imv) -- nice and minimalistic, great wayland support, but not enough configuration options available, e.g. it lacks configuring floating point scaling value (only integer values are supported). Also, like `qimgv` -  project is a bit dead, though it still recieves useful PRs and feedback
- [oculante](https://github.com/woelper/oculante) -- amazing viewer with extremely friendly and passionate maintainer. Supports all modern OSes, works great on wayland, nice performance and so on. As for cons, there's no binary package for it and it's build time is VERY slow and resource-consuming because of heavy dependency tree. The supported format list is also a bit smaller than the `qimgv`'s for example

Of course there are more tools for this purpose here and there, but I didn't like those for a different reasons, so didn't use them for quite enough time to remember

For me, the motivation was is that I wanted an `oculante`, which would be faster to compile, without features I don't need and with simple code base, so I can fix it right away or file a pr/issue in appropriate place. That's it. It handles formats I need, it handles wayland natively, it's performance is not bad and it has config file with all the options that I use. It will *probably* even work on [RedoxOS](https://github.com/woelper/oculante) without changes to a code base!

## Usage

`imgvwr -i image.png`

## Config

Config file isn't created automatically. If you want to use your own config, copy repo's `config.toml` file into `~/.config/imgvwr/config.toml` and edit it. Or you can copy it elsewhere and pass via arguments: `imgvwr -i image.png -c config.toml`

See the `config.toml` file for possible values and explanations

## I want %feature%

I'd prefer to implement new features in upstream projects and provide a PR there in order to keep this project's code as simple as possible, so in *most* cases your issues are probably realted to something upstream. I also don't want to add any more external dependencies, at least for now, so keep it in mind. But feel free to ask anything in issues here too, I'm not against it in any way

As for common issues:

- if it's related to supported image formats, then you should check out the [image](https://github.com/image-rs/image) repo first. If the desired image format isn't supported in `image` - you can file a new issue there. Format's support in `iced` *may* be important too though, since current implementation relies on their format support (which relies on `image` crate), but AFAIK we may have an opportunity to load bytes directly after iced #2356 merges

- if it's related to image-viewing capabilities, you should check out the [iced](https://github.com/iced-rs/iced) repo first, if your feature is implemented there. E.g. you want more `FilterMethod`s to be available. If there are such methods implemented in `iced` and not in `imgvwr` - file an issue/PR here and I'll add the support for it. If it's not - file an issue or PR into `iced` repo for this feature to be implemented there first

- if it's related to image modifications (paint, crop, etc) or a directory view then it's not planned

## TODO

- `FilterMethod` support for `Viewer` -> iced #2324
- `ContentFit` support for `Viewer` -> iced #2330
- rotate `widget` instead of `image` to improve perf/memory consuming -> iced #2334 + impl for `viewer` (merge #2330 and #2334 first)
- moar perf improvements -> iced #2382
- [repo] ci/cd
- [repo] dependabot
