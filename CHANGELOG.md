# Changelog

## [0.2.3](https://github.com/sebastian-software/ferrolink/compare/ferrolink-v0.2.2...ferrolink-v0.2.3) (2026-11-19)


### Features

* audit hreflang in built HTML ([a5e251d](https://github.com/sebastian-software/ferrolink/commit/a5e251d5f85f99436dfd8aacb4f196595f876dda))
* audit static builds for search ([#34](https://github.com/sebastian-software/ferrolink/issues/34)) ([9438f41](https://github.com/sebastian-software/ferrolink/commit/9438f410de5c6950ac53ac7163df2aa8b3d22498))


### Bug Fixes

* resolve encoded dot segments before checking links ([#39](https://github.com/sebastian-software/ferrolink/issues/39)) ([5237884](https://github.com/sebastian-software/ferrolink/commit/523788485f26262cd41af55de0c38d70a4cfaace))
* resolve static deployment links and image preloads ([a768e7b](https://github.com/sebastian-software/ferrolink/commit/a768e7bf948e4769fd87305d24c98ee41529753b))

## [0.2.2](https://github.com/sebastian-software/ferrolink/compare/ferrolink-v0.2.1...ferrolink-v0.2.2) (2026-11-10)


### Features

* GitHub as the header's mark, on Ardo 6.4 ([#25](https://github.com/sebastian-software/ferrolink/issues/25)) ([00d3a54](https://github.com/sebastian-software/ferrolink/commit/00d3a54bb11ad72b408960158de695ab89fdffee))


### Bug Fixes

* keep table text off the frame ([#27](https://github.com/sebastian-software/ferrolink/issues/27)) ([aa73dd5](https://github.com/sebastian-software/ferrolink/commit/aa73dd597d4248df6f22966cfc7d730675e97e9d))
* resolve root-relative links against external bases ([#30](https://github.com/sebastian-software/ferrolink/issues/30)) ([8e7fd5a](https://github.com/sebastian-software/ferrolink/commit/8e7fd5a95dc3487547b014e5b1a368cc93e49862))

## [0.2.1](https://github.com/sebastian-software/ferrolink/compare/ferrolink-v0.2.0...ferrolink-v0.2.1) (2026-10-23)


### Features

* export formatIssues from the JavaScript API ([e605275](https://github.com/sebastian-software/ferrolink/commit/e605275287325fc4ca707916ca1d25d27a0312b9))
* let assume-present globs cover anchors and paths outside the base ([9f58549](https://github.com/sebastian-software/ferrolink/commit/9f58549a3fe769e6fc4b406d02c591047bb25f3b))

## [0.2.0](https://github.com/sebastian-software/ferrolink/compare/ferrolink-v0.1.0...ferrolink-v0.2.0) (2026-10-23)


### ⚠ BREAKING CHANGES

* `Config::base_path` is an `Option<String>` now, and `Config` and `Error` are `#[non_exhaustive]`. Build a `Config` with `Config::new` and set its fields. ([9c7e91e](https://github.com/sebastian-software/ferrolink/commit/9c7e91e770ffb34d85f7f1f7bdabce92d8e655aa))

### Features

* accept targets assumed present and allow turning anchor checks off ([9c7e91e](https://github.com/sebastian-software/ferrolink/commit/9c7e91e770ffb34d85f7f1f7bdabce92d8e655aa))
* publish ferrolink to npm with a JavaScript API and a Vite plugin ([631dfbe](https://github.com/sebastian-software/ferrolink/commit/631dfbed8c7b005d2c94fda0bbeaa8c4879b3ab9))

## 0.1.0 (2026-10-23)


### Features

* check links, assets, and anchors in static HTML builds ([7c0fca4](https://github.com/sebastian-software/ferrolink/commit/7c0fca453db8cc56752b28980961ee532688cad9))
* scan with Ferroni by default ([4330f99](https://github.com/sebastian-software/ferrolink/commit/4330f99e0b4f85be08b5a9fab036cfa7ef2d6433))


### Performance Improvements

* split attributes on the find path ([cb42b5b](https://github.com/sebastian-software/ferrolink/commit/cb42b5b11b4f1bcfcba67a15df63595201f2d34c))
* tune the Ferroni regex backend to html5gum's speed ([6dcd759](https://github.com/sebastian-software/ferrolink/commit/6dcd75956dcade52c5db802caacfa844498d287b))
