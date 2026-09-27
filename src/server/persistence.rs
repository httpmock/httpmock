use std::{
    convert::{TryFrom, TryInto},
    fs::{canonicalize, read, read_dir, read_to_string},
    path::{Component, Path, PathBuf},
};

use bytes::{BufMut, Bytes, BytesMut};
use serde::Deserialize;
use serde_yaml::Deserializer;
use thiserror::Error;

use crate::{
    common::{
        data,
        data::{MockDefinition, StaticMockDefinition},
        util::HttpMockBytes,
    },
    server::state,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("cannot read static mock directory '{}': {source}", path.display())]
    DirectoryRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot read static mock file '{}': {source}", path.display())]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid body_from_file reference '{reference}': {reason}")]
    BodyFromFile { reference: String, reason: String },
    #[error("state operation failed: {0}")]
    State(#[from] state::Error),
    #[error("cannot process YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("cannot convert data structures: {0}")]
    DataConversion(#[from] data::Error),
}

pub fn read_static_mock_definitions(path_opt: PathBuf, state: &state::Manager) -> Result<(), Error> {
    for def in read_static_mocks(path_opt)? {
        state.add_mock(def, true)?;
    }

    Ok(())
}

fn read_static_mocks(path: PathBuf) -> Result<Vec<MockDefinition>, Error> {
    let mut definitions = Vec::new();

    let mock_directory = canonicalize(&path).map_err(|source| Error::DirectoryRead {
        path: path.clone(),
        source,
    })?;

    let paths = read_dir(&mock_directory).map_err(|source| Error::DirectoryRead {
        path: path.clone(),
        source,
    })?;
    for file_path in paths {
        let file_path = file_path
            .map_err(|source| Error::DirectoryRead {
                path: path.clone(),
                source,
            })?
            .path();
        if let Some(ext) = file_path.extension()
            && !"yaml".eq(ext)
            && !"yml".eq(ext)
        {
            continue;
        }

        tracing::info!("Loading static mock file from '{}'", file_path.to_string_lossy());

        let content = read_to_string(&file_path).map_err(|source| Error::FileRead {
            path: file_path.clone(),
            source,
        })?;

        for mut definition in deserialize_mock_defs_from_yaml(&content)? {
            // Resolve file references here, where the YAML file location is known, so that the
            // conversion in the data layer stays free of filesystem access.
            let body_from_file = definition.take_body_from_file();

            let mut mock_definition: MockDefinition = definition.try_into()?;

            if let Some(reference) = body_from_file {
                let bytes = read_mock_file(&reference, &mock_directory, &file_path)?;
                mock_definition.response.body = Some(HttpMockBytes::from(bytes));
            }

            definitions.push(mock_definition);
        }
    }

    Ok(definitions)
}

fn read_mock_file(reference: &str, mock_directory: &Path, definition_file: &Path) -> Result<Vec<u8>, Error> {
    let invalid = |reason: String| Error::BodyFromFile {
        reference: reference.to_string(),
        reason,
    };

    let reference_path = Path::new(reference);
    if reference_path
        .components()
        .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return Err(invalid("the path must be relative".to_string()));
    }

    let path = definition_file.parent().unwrap_or(mock_directory).join(reference_path);
    let path = canonicalize(&path).map_err(|source| Error::FileRead { path, source })?;

    if !path.starts_with(mock_directory) || !path.is_file() {
        return Err(invalid(format!(
            "the path must point to a file inside '{}'",
            mock_directory.display()
        )));
    }

    read(&path).map_err(|source| Error::FileRead { path, source })
}

pub fn deserialize_mock_defs_from_yaml(yaml_content: &str) -> Result<Vec<StaticMockDefinition>, Error> {
    let mut definitions = Vec::new();

    for document in Deserializer::from_str(yaml_content) {
        definitions.push(StaticMockDefinition::deserialize(document)?);
    }

    Ok(definitions)
}

pub fn serialize_mock_defs_to_yaml(mocks: &[MockDefinition]) -> Result<Bytes, Error> {
    let mut buffer = BytesMut::new();

    for (idx, mock) in mocks.iter().enumerate() {
        if idx > 0 {
            buffer.put_slice(b"---\n");
        }

        let static_mock = StaticMockDefinition::try_from(mock)?;
        let yaml = serde_yaml::to_string(&static_mock)?;
        buffer.put_slice(yaml.as_bytes());
    }

    Ok(buffer.freeze())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use crate::{common::data::MockDefinition, server::state};

    use super::{deserialize_mock_defs_from_yaml, read_static_mock_definitions};

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "httpmock-static-body-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn mock_yaml(response: &str) -> String {
        format!("when:\n  path: /test\nthen:\n  status: 200\n  {response}\n")
    }

    #[test]
    fn json_body_is_converted_to_response_bytes() {
        let yaml = r#"when:
  method: GET
  path: /health
then:
  status: 200
  json_body: |-
    {
      "status": "healthy"
    }
"#;
        let definition = deserialize_mock_defs_from_yaml(yaml).unwrap().pop().unwrap();
        let mock: MockDefinition = definition.try_into().unwrap();

        assert_eq!(mock.response.body.unwrap().as_ref(), br#"{"status":"healthy"}"#);
    }

    #[test]
    fn body_from_file_is_rejected_without_a_static_mock_directory() {
        let definition = deserialize_mock_defs_from_yaml(&mock_yaml("body_from_file: payload.bin"))
            .unwrap()
            .pop()
            .unwrap();

        let error = match TryInto::<MockDefinition>::try_into(definition) {
            Ok(_) => panic!("expected the conversion to fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("static mock directory"));
    }

    #[test]
    fn body_from_file_is_relative_and_confined_to_the_mock_directory() {
        let directory = TestDirectory::new();
        let mocks = directory.path().join("mocks");
        fs::create_dir(&mocks).unwrap();
        let payload = [0x00, 0x80, 0xff, b'A'];
        fs::write(mocks.join("payload.bin"), payload).unwrap();
        let yaml_file = mocks.join("mock.yaml");
        fs::write(&yaml_file, mock_yaml("body_from_file: payload.bin")).unwrap();

        let state = state::Manager::default();
        read_static_mock_definitions(mocks.clone(), &state).unwrap();
        let body = state.read_mock(0).unwrap().unwrap().definition.response.body.unwrap();
        assert_eq!(body.as_ref(), payload);

        fs::write(directory.path().join("outside.bin"), b"secret").unwrap();
        fs::write(&yaml_file, mock_yaml("body_from_file: ../outside.bin")).unwrap();
        assert!(read_static_mock_definitions(mocks.clone(), &state::Manager::default()).is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(directory.path().join("outside.bin"), mocks.join("outside-link.bin")).unwrap();
            fs::write(&yaml_file, mock_yaml("body_from_file: outside-link.bin")).unwrap();
            assert!(read_static_mock_definitions(mocks, &state::Manager::default()).is_err());
        }
    }
}
