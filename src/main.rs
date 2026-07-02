mod auth;
mod config;
mod dns;
mod ip;
mod rng;

use std::path::PathBuf;
use std::time::Duration;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config_path = std::env::var("CONFIG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::args()
                .nth(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("config.json"))
        });

    let cfg = config::Config::load(&config_path)?;
    log::info!(
        "loaded config: {} records, interval={}s",
        cfg.records.len(),
        cfg.interval_secs()
    );

    let sa = auth::ServiceAccount::load(&cfg.credentials_path())?;
    log::info!(
        "authenticated as {} (project {})",
        sa.client_email,
        sa.project_id
    );
    let token = auth::TokenProvider::new(sa)?;

    let agent: Agent = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::WebPki)
                .build(),
        )
        .build()
        .into();

    let interval = Duration::from_secs(cfg.interval_secs().max(30));
    let needs_v4 = cfg.needs_v4();
    let needs_v6 = cfg.needs_v6();

    loop {
        let v4 = if needs_v4 {
            match ip::fetch(&agent, dns::v4_endpoint()) {
                Ok(ip) => {
                    log::debug!("public v4: {ip}");
                    Some(ip)
                }
                Err(e) => {
                    log::warn!("fetch v4 failed: {e}");
                    None
                }
            }
        } else {
            None
        };
        let v6 = if needs_v6 {
            match ip::fetch(&agent, dns::v6_endpoint()) {
                Ok(ip) => {
                    log::debug!("public v6: {ip}");
                    Some(ip)
                }
                Err(e) => {
                    log::warn!("fetch v6 failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        for rec in &cfg.records {
            let Some(ip) = dns::pick_ip(rec, v4, v6) else {
                log::warn!(
                    "{} {}: no IP available for source {:?}, skipping",
                    rec.name,
                    rec.rtype(),
                    rec.ip_source
                );
                continue;
            };
            if let Err(e) = dns::update_record(&agent, &token, &cfg, rec, ip) {
                log::error!("{} {}: update failed: {e}", rec.name, rec.rtype());
            }
        }

        std::thread::sleep(interval);
    }
}
