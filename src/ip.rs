use std::net::IpAddr;

pub fn fetch(agent: &ureq::Agent, endpoint: &str) -> Result<IpAddr, Box<dyn std::error::Error>> {
    let body = agent.get(endpoint).call()?.body_mut().read_to_string()?;
    let trimmed = body.trim();
    trimmed
        .parse::<IpAddr>()
        .map_err(|e| format!("invalid IP from {endpoint}: {e}: {trimmed:?}").into())
}
