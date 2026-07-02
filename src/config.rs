use miniserde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub credentials_file: String,
    pub managed_zone: String,
    pub interval_secs: Option<u64>,
    pub records: Vec<Record>,
}

impl Config {
    pub fn credentials_path(&self) -> PathBuf {
        PathBuf::from(&self.credentials_file)
    }

    pub fn interval_secs(&self) -> u64 {
        self.interval_secs.unwrap_or(300)
    }

    pub fn load(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config {}: {e}", path.display()))?;
        let cfg: Config =
            miniserde::json::from_str(&text).map_err(|e| format!("invalid JSON config: {e}"))?;
        if cfg.records.is_empty() {
            return Err("no \"records\" defined in config".into());
        }
        for r in &cfg.records {
            if !r.name.ends_with('.') {
                return Err(
                    format!("record name '{}' must be a FQDN ending with '.'", r.name).into(),
                );
            }
        }
        Ok(cfg)
    }

    /// Does any record need the IPv4 endpoint?
    pub fn needs_v4(&self) -> bool {
        self.records
            .iter()
            .any(|r| matches!(r.ip_source, IpSource::v4))
    }

    /// Does any record need the IPv6 endpoint?
    pub fn needs_v6(&self) -> bool {
        self.records
            .iter()
            .any(|r| matches!(r.ip_source, IpSource::v6))
    }
}

#[derive(Debug, Deserialize)]
pub struct Record {
    pub name: String,
    pub ttl: Option<i64>,
    pub ip_source: IpSource,
}

impl Record {
    pub fn ttl(&self) -> i64 {
        self.ttl.unwrap_or(300)
    }

    pub fn rtype(&self) -> &'static str {
        match self.ip_source {
            IpSource::v4 => "A",
            IpSource::v6 => "AAAA",
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum IpSource {
    v4,
    v6,
}
