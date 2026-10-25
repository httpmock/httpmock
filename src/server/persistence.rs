use std::{
    fs::{canonicalize, read, read_dir, read_to_string},
    path::{Component, Path, PathBuf},
};

use bytes::Bytes;
use thiserror::Error;

use crate::{
    common::{data, data::MockDefinition, static_mock::StaticMockDefinition, util::HttpMockBytes},
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
    #[error("cannot parse YAML: {0}")]
    YamlParse(#[from] serde_saphyr::Error),
    #[error("cannot write YAML: {0}")]
    YamlWrite(#[from] serde_saphyr::SerializeError),
    #[error("cannot convert data structures: {0}")]
    DataConversion(#[from] data::Error),
}

pub fn read_static_mock_definitions(path_opt: PathBuf, state: &state::HttpMockStateManager) -> Result<(), Error> {
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
    // Only `true` and `false` are booleans, like in YAML 1.2 and with serde_yaml before, so an
    // unquoted `yes` or `on` in a `json_body` mapping stays a string.
    let options = serde_saphyr::options! { strict_booleans: true };
    Ok(serde_saphyr::from_multiple_with_options(yaml_content, options)?)
}

pub fn serialize_mock_defs_to_yaml(mocks: &[MockDefinition]) -> Result<Bytes, Error> {
    let static_mocks = mocks
        .iter()
        .map(StaticMockDefinition::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Bytes::from(serde_saphyr::to_string_multiple(&static_mocks)?))
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
    fn yaml_1_1_booleans_stay_strings_in_json_body() {
        let yaml = "when:\n  path: /flags\nthen:\n  status: 200\n  json_body:\n    enabled: yes\n    mode: on\n";
        let definition = deserialize_mock_defs_from_yaml(yaml).unwrap().pop().unwrap();
        let mock: MockDefinition = definition.try_into().unwrap();

        assert_eq!(
            mock.response.body.unwrap().as_ref(),
            br#"{"enabled":"yes","mode":"on"}"#
        );
    }

    #[test]
    fn empty_documents_are_skipped() {
        let yaml = format!("---\n{}---\n---\n{}---\n", mock_yaml("delay: 1"), mock_yaml("delay: 2"));
        assert_eq!(deserialize_mock_defs_from_yaml(&yaml).unwrap().len(), 2);
    }

    #[test]
    fn up_to_n_times_is_read_from_yaml() {
        let definition = deserialize_mock_defs_from_yaml(&mock_yaml("up_to_n_times: 2"))
            .unwrap()
            .pop()
            .unwrap();
        let mock: MockDefinition = definition.try_into().unwrap();

        assert_eq!(mock.up_to_n_times, Some(2));
    }

    #[test]
    fn zero_up_to_n_times_is_rejected_in_static_mock_directory() {
        let directory = TestDirectory::new();
        fs::write(directory.path().join("mock.yaml"), mock_yaml("up_to_n_times: 0")).unwrap();

        let error =
            read_static_mock_definitions(directory.path().to_path_buf(), &state::HttpMockStateManager::default())
                .expect_err("expected loading the mock to fail");
        assert!(error.to_string().contains("up_to_n_times must be greater than 0"));
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

        let state = state::HttpMockStateManager::default();
        read_static_mock_definitions(mocks.clone(), &state).unwrap();
        let body = state.read_mock(0).unwrap().unwrap().definition.response.body.unwrap();
        assert_eq!(body.as_ref(), payload);

        fs::write(directory.path().join("outside.bin"), b"secret").unwrap();
        fs::write(&yaml_file, mock_yaml("body_from_file: ../outside.bin")).unwrap();
        assert!(read_static_mock_definitions(mocks.clone(), &state::HttpMockStateManager::default()).is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(directory.path().join("outside.bin"), mocks.join("outside-link.bin")).unwrap();
            fs::write(&yaml_file, mock_yaml("body_from_file: outside-link.bin")).unwrap();
            assert!(read_static_mock_definitions(mocks, &state::HttpMockStateManager::default()).is_err());
        }
    }
}
