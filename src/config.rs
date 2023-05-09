use config::FileFormat;
use config::{Config as Conf, ConfigError, File};
use directories::ProjectDirs;
use serde::{de, Deserialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct Core {
    #[serde(default, deserialize_with = "deserialize_bool")]
    pub no_scm: bool,
    #[serde(default)]
    pub site_cache_dir: Option<PathBuf>,
    #[serde(default)]
    pub checksum_jobs: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(unused)]
pub struct Cache {
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(rename = "type", default)]
    pub typ: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
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
        Conf::builder()
            .add_source(
                File::from(
                    ProjectDirs::from("", "iterative", "dvc")
                        .unwrap()
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
            .try_deserialize()
    }
}
