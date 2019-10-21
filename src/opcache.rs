use chrono::{DateTime, Utc};
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Opcache {
    pub opcache_enabled: bool,
    pub cache_full: bool,
    pub restart_pending: bool,
    pub restart_in_progress: bool,
    pub memory_usage: MemoryUsage,
    pub interned_strings_usage: InternedStringsUsage,
    pub opcache_statistics: OpcacheStatistics,
    pub scripts: HashMap<PathBuf, Scripts>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MemoryUsage {
    pub used_memory: isize,
    pub free_memory: isize,
    pub wasted_memory: isize,
    pub current_wasted_percentage: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InternedStringsUsage {
    pub buffer_size: isize,
    pub used_memory: isize,
    pub free_memory: isize,
    pub number_of_strings: isize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OpcacheStatistics {
    pub num_cached_scripts: isize,
    pub num_cached_keys: isize,
    pub max_cached_keys: isize,
    pub hits: isize,
    pub start_time: isize,
    pub last_restart_time: isize,
    pub oom_restarts: isize,
    pub hash_restarts: isize,
    pub manual_restarts: isize,
    pub misses: isize,
    pub blacklist_misses: isize,
    pub blacklist_miss_ratio: f64,
    pub opcache_hit_rate: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Scripts {
    pub full_path: PathBuf,
    pub hits: isize,
    pub memory_consumption: f64,
    #[serde(with = "php_date_format")]
    pub last_used: DateTime<Utc>,
    pub last_used_timestamp: isize,
    pub timestamp: isize,
}

mod php_date_format {
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &str = "%a %b %e %T %Y";

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Utc.datetime_from_str(&s, FORMAT)
            .map_err(serde::de::Error::custom)
    }
}
