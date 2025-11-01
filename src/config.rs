use config::FileFormat;
use config::{Config as Conf, File};
use directories::ProjectDirs;
use serde::{Deserialize, de};
use serde_json::Value;
use serde_with::StringWithSeparator;
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use std::path::{Path, PathBuf};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum ConfigError {
    #[error(transparent)]
    ConfigError(#[from] config::ConfigError),
    #[error("failed to determine home directory")]
    FailedToGetNoHomeDir,
    #[error(transparent)]
    SerdeError(#[from] serde::de::value::Error),
}

#[derive(Debug, Deserialize, Default)]
pub struct Core {
    #[serde(default, deserialize_with = "deserialize_bool")]
    pub no_scm: bool,
    #[serde(default)]
    pub site_cache_dir: Option<PathBuf>,
    #[serde(default)]
    pub checksum_jobs: Option<usize>,
}

#[serde_as]
#[derive(Debug, Deserialize, Default)]
pub struct Cache {
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(rename = "type", default)]
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    pub typ: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub core: Core,
    #[serde(default)]
    pub cache: Cache,
}

fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: de::Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Bool(v) => Ok(v),
        Value::String(s) => {
            if s == "true" {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        t => Err(de::Error::custom(format!("expected boolean, got {t}"))),
    }
}

impl Config {
    pub fn new(control_dir: &Path) -> Result<Self, ConfigError> {
        let conf = Conf::builder()
            .add_source(
                File::from(
                    ProjectDirs::from("", "iterative", "dvc")
                        .ok_or_else(|| ConfigError::FailedToGetNoHomeDir)?
                        .config_local_dir()
                        .join("config"),
                )
                .required(false)
                .format(FileFormat::Ini),
            )
            .add_source(
                File::from(control_dir.join("config"))
                    .required(false)
                    .format(FileFormat::Ini),
            )
            .add_source(
                File::from(control_dir.join("config.local"))
                    .required(false)
                    .format(FileFormat::Ini),
            )
            .build()?
            .try_deserialize()?;
        Ok(conf)
    }
}
