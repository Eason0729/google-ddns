use crate::auth::TokenProvider;
use crate::config::{Config, IpSource, Record};
use miniserde::Deserialize;
use std::net::IpAddr;

const V4_ENDPOINT: &str = "https://ipv4.icanhazip.com";
const V6_ENDPOINT: &str = "https://ipv6.icanhazip.com";

pub fn v4_endpoint() -> &'static str {
    V4_ENDPOINT
}

pub fn v6_endpoint() -> &'static str {
    V6_ENDPOINT
}

#[derive(Debug, Deserialize)]
struct RrsetResp {
    rrdata: Option<Vec<String>>,
    ttl: Option<i64>,
}

fn base_url(project: &str, zone: &str, name: &str, rtype: &str) -> String {
    format!(
        "https://dns.googleapis.com/dns/v1/projects/{project}/managedZones/{zone}/rrsets/{name}/{rtype}"
    )
}

fn create_url(project: &str, zone: &str) -> String {
    format!("https://dns.googleapis.com/dns/v1/projects/{project}/managedZones/{zone}/rrsets")
}

fn rrset_body(name: &str, rtype: &str, ttl: i64, rrdata: &[String]) -> String {
    let data = rrdata
        .iter()
        .map(|d| format!("\"{}\"", d.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"name":"{name}","type":"{rtype}","ttl":{ttl},"rrdata":[{data}]}}"#,
        name = name.replace('"', "\\\""),
    )
}

pub fn update_record(
    agent: &ureq::Agent,
    token: &TokenProvider,
    cfg: &Config,
    rec: &Record,
    ip: IpAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let access = token.access_token(agent)?;
    let project = token.project_id();
    let zone = &cfg.managed_zone;
    let rtype = rec.rtype();
    let rrdata = vec![ip.to_string()];
    let ttl = rec.ttl();

    let get_url = base_url(project, zone, &rec.name, rtype);
    let get_resp = agent
        .get(&get_url)
        .header("Authorization", format!("Bearer {access}"))
        .call();

    match get_resp {
        Ok(mut r) => {
            let body = r.body_mut().read_to_string()?;
            let existing: RrsetResp = miniserde::json::from_str(&body)
                .map_err(|e| format!("parse GET rrset: {e}: {body}"))?;

            let cur = existing.rrdata.unwrap_or_default();
            let cur_ttl = existing.ttl.unwrap_or(0);
            if cur.len() == 1 && cur[0] == rrdata[0] && cur_ttl == ttl {
                log::info!("{} {} up to date ({})", rec.name, rtype, rrdata[0]);
                return Ok(());
            }

            log::info!(
                "{} {} updating -> {} (ttl {})",
                rec.name,
                rtype,
                rrdata[0],
                ttl
            );
            let body = rrset_body(&rec.name, rtype, ttl, &rrdata);
            agent
                .patch(&get_url)
                .header("Authorization", format!("Bearer {access}"))
                .header("Content-Type", "application/json")
                .send(body)?
                .body_mut()
                .read_to_string()?;
        }
        Err(ureq::Error::StatusCode(404)) => {
            log::info!(
                "{} {} creating -> {} (ttl {})",
                rec.name,
                rtype,
                rrdata[0],
                ttl
            );
            let body = rrset_body(&rec.name, rtype, ttl, &rrdata);
            agent
                .post(create_url(project, zone))
                .header("Authorization", format!("Bearer {access}"))
                .header("Content-Type", "application/json")
                .send(body)?
                .body_mut()
                .read_to_string()?;
        }
        Err(e) => return Err(format!("GET rrset failed: {e}").into()),
    }
    Ok(())
}

pub fn pick_ip(rec: &Record, v4: Option<IpAddr>, v6: Option<IpAddr>) -> Option<IpAddr> {
    match rec.ip_source {
        IpSource::v4 => v4,
        IpSource::v6 => v6,
    }
}
