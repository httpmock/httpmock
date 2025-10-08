# Changelog

## Version 0.8.2 (unreleased / in development)

The following pull requests have been merged:
- [#178](https://github.com/httpmock/httpmock/pull/178): "Expose proxy method to obtain the recorded yaml without saving to a file" (thanks [@janeisklar](https://github.com/janeisklar))
- [#180](https://github.com/httpmock/httpmock/pull/180): "Add missing query parameters in recordings"
- [#181](https://github.com/httpmock/httpmock/pull/181): "Append Headers Instead of Inserting"

## Version 0.8.1
This release includes bug fixes and documentation enhancements.

The following pull requests have been merged:
- [#179](https://github.com/httpmock/httpmock/pull/179): "Use scheme of target url for forwarding"

## Version 0.8.0
This release includes refactoring, dependency updates, and internal cleanups.
No breaking changes expected.

The minimum required Rust version has been increased to 1.82.

The following pull requests have been merged:
- [#172](https://github.com/httpmock/httpmock/pull/172): "Update Rust edition to 2021" (thanks [@FalkWoldmann](https://github.com/FalkWoldmann))
- [#169](https://github.com/httpmock/httpmock/pull/169): "Proxy HTTPS fix"
- [#167](https://github.com/httpmock/httpmock/pull/167): "Replace log and env_logger with tracing and tracing-subscriber"  (thanks [@FalkWoldmann](https://github.com/FalkWoldmann))
- [#166](https://github.com/httpmock/httpmock/pull/166): "Remove unused code" (thanks [@FalkWoldmann](https://github.com/FalkWoldmann))
- [#163](https://github.com/httpmock/httpmock/pull/163): "fix: issue 162, non localhost hosts match" (thanks [@Thomblin](https://github.com/Thomblin))
- [#160](https://github.com/httpmock/httpmock/pull/160): "Replace custom read_file with std::fs::read_to_string" (thanks [@FalkWoldmann](https://github.com/FalkWoldmann))
- [#158](https://github.com/httpmock/httpmock/pull/158): "Improve async executor support"
- [#156](https://github.com/httpmock/httpmock/pull/156): "Bump async-object-pool to replace async-std"
- [#153](https://github.com/httpmock/httpmock/pull/153): "ci(deps): bump actions/checkout from 4 to 5"
- [#152](https://github.com/httpmock/httpmock/pull/152): "Fix missing standalone routes"
- [#151](https://github.com/httpmock/httpmock/pull/151): "Cleanup unused test functions"
- [#147](https://github.com/httpmock/httpmock/pull/147): "ci(deps): bump codecov/codecov-action from 2 to 5"
- [#146](https://github.com/httpmock/httpmock/pull/146): "cargo(deps): update thiserror requirement from 1 to 2"
- [#145](https://github.com/httpmock/httpmock/pull/145): "ci(deps): bump actions/checkout from 2 to 4"
- [#144](https://github.com/httpmock/httpmock/pull/144): "ci(deps): bump docker/build-push-action from 4 to 6
- [#141](https://github.com/httpmock/httpmock/pull/141): "cargo(deps): update path-tree requirement from >=0.8.0, <0.8.1 to >=0.8.0, <0.8.4"
- [#140](https://github.com/httpmock/httpmock/pull/140): "ci(deps): bump docker/login-action from 1 to 3"
- [#139](https://github.com/httpmock/httpmock/pull/139): "ci(deps): bump withastro/action from 2 to 4"
- [#138](https://github.com/httpmock/httpmock/pull/138): "Create dependabot.yml" (thanks [@FalkWoldmann](https://github.com/FalkWoldmann))
- [#136](https://github.com/httpmock/httpmock/pull/136): "Replace async_std with tokio" (thanks [@FalkWoldmann](https://github.com/FalkWoldmann))

## Version 0.8.0-beta.1
This release mainly contains internal improvements and bugfixes.
The minimum required Rust version has been increased to 1.81.
Apart from the updated MSRV, there are no breaking changes.

The following pull requests have been merged:
- [#112](https://github.com/httpmock/httpmock/pull/112): "Fix building without cookies feature" by [@jayvdb](https://github.com/jayvdb).
- [#117](https://github.com/httpmock/httpmock/pull/117): "fix rustls crypto provider features" by [@Taowyoo](https://github.com/Taowyoo).
- [#120](https://github.com/httpmock/httpmock/pull/120): "Refactoring and cleanup". THanks by [@FalkWoldmann](https://github.com/FalkWoldmann).

## Version 0.8.0-alpha.1

### BREAKING CHANGES
- A new [MockServer::reset](https://docs.rs/httpmock/latest/httpmock/struct.MockServer.html#method.reset) method was added that resets a mock server. Thanks for providing the [pull request](https://github.com/httpmock/httpmock/pull/100) for this feature, [@dax](https://github.com/dax).
- The default port for standalone server was changed from `5000` to `5050` due to conflicts with system services on macOS.
- [Custom matcher functions](https://docs.rs/httpmock/latest/httpmock/struct.When.html#method.matches) are now closures rather than functions.
- [When::json_body_partial](https://docs.rs/httpmock/0.7.0/httpmock/struct.When.html#method.json_body_partial) was renamed to `json_body_includes`.
- [When::x_www_form_urlencoded_tuple](https://docs.rs/httpmock/0.7.0/httpmock/struct.When.html#method.x_www_form_urlencoded) was renamed to `form_urlencoded_tuple`.
- [When::x_www_form_urlencoded_key_exists](https://docs.rs/httpmock/0.7.0/httpmock/struct.When.html#method.x_www_form_urlencoded) was renamed to `form_urlencoded_key_exists`.
- Error message output has been changed for better readability (e.g., when calling `Mock::assert`).
- Custom matcher function `When::matches` has been renamed to `When::is_true`.

#### Improvements
- Record and Playback mode was added
- Many new matchers functions have been added
- Proxy Mode was added
- Website docs have been created (see https://httpmock.rs)
- HTTPS support added
- Internal implementation was entirely rewritten

### Improvements
- The algorithm to find the most similar request in case of mock assertion failures has been improved.

## Version 0.7.0

- **BREAKING CHANGES**:
  - For connecting to **remote** `httpmock` servers during tests using any of the `connect` methods like
    [MockServer::connect](https://docs.rs/httpmock/latest/httpmock/struct.MockServer.html#method.connect),
    [MockServer::connect_async](https://docs.rs/httpmock/latest/httpmock/struct.MockServer.html#method.connect_async),
    [MockServer::connect_from_env](https://docs.rs/httpmock/latest/httpmock/struct.MockServer.html#method.connect_from_env), or
    [MockServer::connect_from_env_async](https://docs.rs/httpmock/latest/httpmock/struct.MockServer.html#method.connect_from_env_async), 
    you must now activate the `remote` feature. This feature is not enabled by default.

- Improvements:
  - The dependency tree has been significantly slimmed down when the `remote` feature is not enabled.
  - If the new `remote` feature is not enabled, `httpmock` no longer has a dependency on a real HTTP client. 
    As a result, certain [TLS issues previously reported by users](https://github.com/httpmock/httpmock/issues/82) 
    should no longer arise.

- This release also updates all dependencies to the most recent version.
- The minimum Rust version has been bumped to 1.70.

## Version 0.6.8

- This is a maintenance release that updates all dependencies to the most recent version.
- Fixes some dependency issues with the Docker image.

## Version 0.6.7

- This is a maintenance release that updates all dependencies to the most recent version.

## Version 0.6.6

- Extended some API methods to allow for more type flexibility (see <https://github.com/httpmock/httpmock/issues/58>). Thanks to [@95th](https://github.com/95th) for providing the PR!
- Fixed parsing query parameter values that contain `+` to represent space (see <https://github.com/httpmock/httpmock/issues/56>). Thanks to [@95th](https://github.com/95th) for providing the PR!
- Added a new Cargo feature `cookie` to shorten compile time (see <https://github.com/httpmock/httpmock/pull/63>). Thanks to [mythmon](https://github.com/mythmon) for providing this PR!

## Version 0.6.5

- Fixes a race condition that could occur when deleting mocks from the mock server (see <https://github.com/httpmock/httpmock/issues/53>).
- Replaced internal diff library (switched from `difference` to `similar`, see <https://github.com/httpmock/httpmock/pull/55>).

## Version 0.6.4

- Fixed minimum Rust version in README (raised from 1.47 to 1.54, see release 0.6.3 for more information).

## Version 0.6.3

- This is a maintenance release that updates all dependencies to the most recent version.
- Bumped minimum Rust version to 1.54 due to transitive dependency.

## Version 0.6.2

- A bug was fixed that has unexported the [When](https://docs.rs/httpmock/0.5.8/httpmock/struct.When.html) and
  [Then](https://docs.rs/httpmock/0.5.8/httpmock/struct.When.html) structures. Both types are now exported again.
  Please refer to <https://github.com/httpmock/httpmock/issues/47> for more info.

## Version 0.6.1

- This is a maintenance release that updates all dependencies to the most recent version.

## Version 0.6.0

### General

- Old [Mock](https://docs.rs/httpmock/0.4.5/httpmock/struct.Mock.html) structure based API was deprecated
  starting from version 0.5.0 and was removed with this version. Please switch to the new API based on the
  [When](https://docs.rs/httpmock/0.5.8/httpmock/struct.When.html) /
  [Then](https://docs.rs/httpmock/0.5.8/httpmock/struct.When.html) structures.
- The two methods `MockRef::times_called` and `MockRef::times_called_async` were deprecated since version 0.5.0 and
  have now been removed.
- A [prelude module](https://github.com/httpmock/httpmock#getting-started) was added to shorten imports
  that are usually required when using `httpmock` in tests.
- The struct `MockRef` has been renamed to `Mock`.
- Trait `MockRefExt` has been renamed to `MockExt`.
- Added support for x-www-form-urlencoded request bodies.

### Standalone Mock Server

- Standalone server now has a request history limit that can be adjusted.
- All standalone servers parameters now have an environment variable fallback.
- Standalone servers `exposed` and `disable_access_log` parameters were changed, so that they now require a value
  in addition to the flag itself (this is due to a limitation of `structopt`/`clap`):
  Before: `httpmock --expose`, Now: `httpmock --expose true`.

## Version 0.5.8

- A bug has been fixed that prevented to use the mock server for requests containing a `multipart/form-data`
  request body with binary data.

## Version 0.5.7

- Added static mock support based on YAML files for standalone mode.
- Dockerfile Rust version has been fixed.
- Documentation on query parameters has been enhanced.
- Bumped minimum Rust version to 1.46 due to transitive dependency.

## Version 0.5.6

- A bug has been fixed that caused false positive warnings in the log output.
- Updated all dependencies to the most recent versions.
- Assertion error messages (`MockRef::assert` and `MockRef::assert_hits`) now contain more details.

## Version 0.5.5

- A bug has been fixed that prevented to use a request body in DELETE requests.

## Version 0.5.4

- A new extension trait `MockRefExt` was added that extends the `MockRef` structure with additional but usually
not required functionality.

## Version 0.5.3

- This is a maintenance release that updates all dependencies to the most recent version.
- This release bumps the minimal Rust version from 1.43+ to 1.45+.

## Version 0.5.2

- Updated dependencies to newest version.
- Removed dependency version fixation from v0.5.1.
- `Mock::return_body_from_file` and `Then::body_from_file` now accept absolute and relative file paths.

## Version 0.5.1

- Updated dependency to futures-util to fix compile errors.
- Fixed all dependency version numbers to avoid future problems with new dependency version releases.

## Version 0.5.0

- ❌ _**Breaking Change**_: Function `Mock::expect_json_body` was renamed to `expect_json_body_obj`.
- ❌ _**Breaking Change**_: Function `Mock::return_json_body` was renamed to `return_json_body_obj`.
- 🚀 _**Attention**: A new API for mock definition was added. The old API is still available and functional,
but is deprecated from now on. Please consider switching to the new API._
- 🚀 **Attention**: The following new assertion functions have been added that will provide you smart and helpful
error output to support debugging:
  - `MockRef::assert`
  - `MockRef::assert_hits`
  - `MockRef::assert_async`
  - `MockRef::assert_hits_async`
- The two methods `MockRef::times_called` and `MockRef::times_called_async` are now deprecated. Consider using
`MockRef::hits` and `MockRef::hits_async`.
- The two methods `Mock::return_body` and `Then::body` now accept binary content.
- The following new methods accept a `serde_json::Value`:
  - `Mock::expect_json_body`
  - `Mock::return_json_body`
  - `When::json_body`
  - `Then::json_body`
- 🔥 Improved documentation (**a lot!**).
- 👏 Debug log output is now pretty printed!
- 🍪 Cookie matching support.
- Support for convenient temporary and permanent redirect.
- The log level of some log messages was changed from `debug` to `trace` to make debugging easier.

## Version 0.4.5

- Improved documentation.
- Added a new function `base_url` to the `MockServer` structure.
