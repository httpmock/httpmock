use std::{
    convert::{TryFrom, TryInto},
    fs::{read_dir, read_to_string},
    path::PathBuf,
};

use bytes::{BufMut, Bytes, BytesMut};
use serde::Deserialize;
use serde_yaml::{Deserializer, Value as YamlValue};
use thiserror::Error;

use crate::{
    common::{
        data,
        data::{MockDefinition, StaticMockDefinition},
    },
    server::{
        persistence::Error::{Deserialization, FileRead},
        state,
        state::{Error::DataConversionError, StateManager},
    },
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("cannot read from mock file: {0}")]
    FileRead(String),
    #[error("cannot modify state: {0}")]
    State(#[from] state::Error),
    #[error("cannot deserialize YAML: {0}")]
    Deserialization(String),
    #[error("cannot convert data structures: {0}")]
    DataConversion(#[from] data::Error),
}

pub fn read_static_mock_definitions<S>(path_opt: PathBuf, state: &S) -> Result<(), Error>
where
    S: StateManager + Send + Sync + 'static,
{
    for def in read_static_mocks(path_opt)? {
        state.add_mock(def.try_into()?, true)?;
    }

    Ok(())
}

fn read_static_mocks(path: PathBuf) -> Result<Vec<StaticMockDefinition>, Error> {
    let mut definitions: Vec<StaticMockDefinition> = Vec::new();

    let paths = read_dir(path).expect("cannot list files in directory");
    for file_path in paths {
        let file_path = file_path.unwrap().path();
        if let Some(ext) = file_path.extension()
            && !"yaml".eq(ext)
            && !"yml".eq(ext)
        {
            continue;
        }

        tracing::info!("Loading static mock file from '{}'", file_path.to_string_lossy());

        let content = read_to_string(file_path).map_err(|err| FileRead(err.to_string()))?;

        definitions.extend(deserialize_mock_defs_from_yaml(&content)?);
    }

    Ok(definitions)
}

pub fn deserialize_mock_defs_from_yaml(yaml_content: &str) -> Result<Vec<StaticMockDefinition>, Error> {
    let mut definitions = Vec::new();

    for document in Deserializer::from_str(yaml_content) {
        let value = YamlValue::deserialize(document).map_err(|err| Deserialization(err.to_string()))?;

        let definition: StaticMockDefinition =
            serde_yaml::from_value(value).map_err(|err| Deserialization(err.to_string()))?;

        definitions.push(definition);
    }

    Ok(definitions)
}

pub fn serialize_mock_defs_to_yaml(mocks: &[MockDefinition]) -> Result<Bytes, Error> {
    let mut buffer = BytesMut::new();

    for (idx, mock) in mocks.iter().enumerate() {
        if idx > 0 {
            buffer.put_slice(b"---\n");
        }

        let static_mock: StaticMockDefinition =
            StaticMockDefinition::try_from(mock).map_err(|err| DataConversionError(err.to_string()))?;
        let yaml = serde_yaml::to_string(&static_mock).map_err(|err| DataConversionError(err.to_string()))?;
        buffer.put_slice(yaml.as_bytes());
    }

    Ok(buffer.freeze())
}
