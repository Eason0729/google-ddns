use crate::auth::TokenProvider;
use crate::config::{Config, IpSource, Record};
use miniserde::Deserialize;
use percent_encoding::{AsciiSet, CONTROLS};
use std::net::IpAddr;

const V4_ENDPOINT: &str = "https://ipv4.icanhazip.com";
const V6_ENDPOINT: &str = "https://ipv6.icanhazip.com";

/// Characters that must be percent-encoded in a URI path segment.
/// Mirrors RFC 3986: keep only unreserved chars (A-Za-z0-9-._~) plus
/// sub-delims that are safe in a path. The `*` in wildcard record names
/// (e.g. `*.example.com.`) MUST be encoded as `%2A` — GCP rejects the
/// literal `*` in the path with "Invalid value for 'entity.rrset.name'".
const PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'*');

fn encode_path_segment(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, PATH_ENCODE_SET).to_string()
}

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
    let name = encode_path_segment(name);
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

/// Read a response body, returning an error that includes GCP's message on failure.
fn read_body(
    resp: ureq::http::Response<ureq::Body>,
    op: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let status = resp.status().as_u16();
    let body = resp.into_body().read_to_string()?;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(format!("{op} failed: http {status}: {body}").into())
    }
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
    let resp = agent
        .get(&get_url)
        .header("Authorization", format!("Bearer {access}"))
        .call()?;

    if resp.status().as_u16() == 404 {
        log::info!(
            "{} {} creating -> {} (ttl {})",
            rec.name,
            rtype,
            rrdata[0],
            ttl
        );
        let body = rrset_body(&rec.name, rtype, ttl, &rrdata);
        let resp = agent
            .post(create_url(project, zone))
            .header("Authorization", format!("Bearer {access}"))
            .header("Content-Type", "application/json")
            .send(body)?;
        read_body(resp, "POST")?;
        return Ok(());
    }

    let body = read_body(resp, "GET")?;
    let existing: RrsetResp =
        miniserde::json::from_str(&body).map_err(|e| format!("parse GET rrset: {e}: {body}"))?;

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
    let resp = agent
        .patch(&get_url)
        .header("Authorization", format!("Bearer {access}"))
        .header("Content-Type", "application/json")
        .send(body)?;
    read_body(resp, "PATCH")?;
    Ok(())
}

pub fn pick_ip(rec: &Record, v4: Option<IpAddr>, v6: Option<IpAddr>) -> Option<IpAddr> {
    match rec.ip_source {
        IpSource::v4 => v4,
        IpSource::v6 => v6,
    }
}
