# Standalone YAML response bodies (#118, #185)

Issues: [#118](https://github.com/httpmock/httpmock/issues/118), [#185](https://github.com/httpmock/httpmock/issues/185)

## Problem

```text
YAML
  -> StaticMockDefinition
  -> MockDefinition
```

`StaticMockDefinition.then` uses `StaticHTTPResponse`. This type did not contain `json_body` or `body_from_file`, so Serde silently discarded both fields and the runtime response had no body.

## Fix

Keep the existing `StaticMockDefinition` and add both missing fields to `StaticHTTPResponse`.

`json_body` is converted to the normal runtime `body`:

```yaml
then:
  json_body: '{ "status": "healthy" }'
```

```text
response.body = {"status":"healthy"}
```

No content-type header is added because the existing Rust `Then::json_body` API does not add one.

`body_from_file` is resolved while the static-directory loader still knows the YAML file location:

```text
device-mock/
├── mock.yaml
└── files/
    └── firmware.bin
```

```yaml
then:
  body_from_file: files/firmware.bin
```

The path is relative to `mock.yaml`. The static-directory loader resolves `body_from_file` itself: it takes the reference out of the parsed definition, converts the rest with the ordinary (IO-free) conversion, reads the file, and sets the runtime response body. The resolved file must remain inside the configured static-mock directory. Absolute paths, `..` escapes, directories, and symlinks resolving outside that directory are rejected. The file is read once and becomes the runtime response body.

Outside the static-directory loader (e.g. mocks imported from a recording via the HTTP API), a definition containing `body_from_file` is rejected with an error stating that the field is only supported for static mock directories.

Recordings continue using the same `StaticMockDefinition`. Export writes runtime bytes as `body` or `body_base64` and never emits `body_from_file`; no recording-specific type or file-loading behavior is added.

## Not included

Unknown-field rejection, response-field conflict validation, changes to Base64 handling, and changes to the public Rust API are separate work.
