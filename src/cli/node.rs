//! `aster_drive node` 的聚合入口。

use crate::config::node_mode::NodeRuntimeMode;
use crate::db::repository::policy_repo;
use crate::errors::{AsterError, Result};
use crate::services::master_binding_service;
use crate::storage::remote_protocol::normalize_remote_base_url;
use crate::types::DriverType;
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use super::shared::{
    CliTerminalPalette, OutputFormat, ResolvedOutputFormat, human_key, prepare_database,
    render_error_json, render_success_envelope,
};

const FOLLOWER_DEFAULT_SERVER_HOST: &str = "0.0.0.0";

#[derive(Debug, Clone, Subcommand)]
pub enum NodeCommand {
    /// Redeem a master-issued enrollment token and write the local master binding
    Enroll(NodeEnrollArgs),
}

#[derive(Debug, Clone, Args)]
pub struct NodeEnrollArgs {
    #[arg(long, env = "ASTER_CLI_MASTER_URL")]
    pub master_url: String,
    #[arg(long, env = "ASTER_CLI_ENROLLMENT_TOKEN")]
    pub token: String,
    #[arg(long, env = "ASTER_CLI_DATABASE_URL")]
    pub database_url: Option<String>,
    #[arg(long, env = "ASTER_CLI_INGRESS_POLICY_ID")]
    pub ingress_policy_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct NodeEnrollReport {
    action: &'static str,
    binding_id: i64,
    binding_name: String,
    master_url: String,
    namespace: String,
    access_key: String,
    ingress_policy_id: i64,
    ingress_policy_name: String,
    config_path: String,
    server_host: String,
    server_port: u16,
    readiness_check_path: String,
    connectivity_hint: String,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    code: i32,
    msg: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct RemoteEnrollmentBootstrap {
    remote_node_name: String,
    master_url: String,
    access_key: String,
    secret_key: String,
    namespace: String,
    is_enabled: bool,
    ack_token: String,
}

pub async fn execute_node_command(command: &NodeCommand) -> Result<NodeEnrollReport> {
    match command {
        NodeCommand::Enroll(args) => execute_enroll(args).await,
    }
}

pub fn render_node_success(format: OutputFormat, report: &NodeEnrollReport) -> String {
    match format.resolve() {
        ResolvedOutputFormat::Json => render_success_envelope(report, false),
        ResolvedOutputFormat::PrettyJson => render_success_envelope(report, true),
        ResolvedOutputFormat::Human => render_node_human(report),
    }
}

pub fn render_node_error(format: OutputFormat, err: &AsterError) -> String {
    match format.resolve() {
        ResolvedOutputFormat::Json => render_error_json(err, false),
        ResolvedOutputFormat::PrettyJson => render_error_json(err, true),
        ResolvedOutputFormat::Human => err.to_string(),
    }
}

async fn execute_enroll(args: &NodeEnrollArgs) -> Result<NodeEnrollReport> {
    let config_path = ensure_follower_start_mode()?;
    let config = crate::config::get_config();
    let master_url = normalize_remote_base_url(&args.master_url)?;
    let database_url = resolve_database_url(args.database_url.as_deref())?;
    let db = prepare_database(&database_url).await?;
    let ingress_policy = resolve_ingress_policy(&db, args.ingress_policy_id).await?;
    let bootstrap = redeem_enrollment(&master_url, &args.token).await?;
    let binding_name = bootstrap.remote_node_name;
    let enrolled_master_url = bootstrap.master_url;
    let access_key = bootstrap.access_key;
    let secret_key = bootstrap.secret_key;
    let namespace = bootstrap.namespace;
    let is_enabled = bootstrap.is_enabled;
    let ack_token = bootstrap.ack_token;

    let (binding, action) = crate::db::transaction::with_transaction(&db, async |txn| {
        master_binding_service::upsert_from_enrollment(
            txn,
            master_binding_service::UpsertMasterBindingInput {
                name: binding_name.clone(),
                master_url: enrolled_master_url.clone(),
                access_key: access_key.clone(),
                secret_key: secret_key.clone(),
                namespace: namespace.clone(),
                ingress_policy_id: ingress_policy.id,
                is_enabled,
            },
        )
        .await
    })
    .await?;
    let binding_id = binding.id;

    ack_enrollment(&master_url, &ack_token).await?;

    Ok(NodeEnrollReport {
        action,
        binding_id,
        binding_name,
        master_url: enrolled_master_url,
        namespace,
        access_key,
        ingress_policy_id: ingress_policy.id,
        ingress_policy_name: ingress_policy.name,
        config_path,
        server_host: config.server.host.clone(),
        server_port: config.server.port,
        readiness_check_path: "/health/ready".to_string(),
        connectivity_hint: build_connectivity_hint(&config.server.host, config.server.port),
    })
}

fn ensure_follower_start_mode() -> Result<String> {
    let default = follower_seed_config();
    let config_path = crate::config::ensure_default_config_for_current_dir(&default)?;
    let config_path = config_path.display().to_string();

    crate::config::init_config()?;
    let config = crate::config::get_config();

    if matches!(
        crate::config::node_mode::start_mode(config.as_ref()),
        NodeRuntimeMode::Follower
    ) {
        return Ok(config_path);
    }

    Err(AsterError::validation_error(format!(
        "before enrolling this node, set [server].start_mode = \"follower\" in {} and rerun the command",
        config_path
    )))
}

fn follower_seed_config() -> crate::config::Config {
    let mut default = crate::config::Config::default();
    default.server.start_mode = NodeRuntimeMode::Follower;
    default.server.host = FOLLOWER_DEFAULT_SERVER_HOST.to_string();
    default
}

fn resolve_database_url(explicit: Option<&str>) -> Result<String> {
    if let Some(database_url) = explicit {
        return Ok(database_url.to_string());
    }

    crate::config::init_config()?;
    Ok(crate::config::get_config().database.url.clone())
}

async fn redeem_enrollment(master_url: &str, token: &str) -> Result<RemoteEnrollmentBootstrap> {
    let url = format!("{master_url}/api/v1/public/remote-enrollment/redeem");
    let response = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .map_err(|error| {
            AsterError::config_error(format!(
                "failed to reach master enrollment endpoint: {error}"
            ))
        })?;

    parse_api_response(response, "master enrollment request").await
}

async fn ack_enrollment(master_url: &str, ack_token: &str) -> Result<()> {
    let url = format!("{master_url}/api/v1/public/remote-enrollment/ack");
    let response = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({ "ack_token": ack_token }))
        .send()
        .await
        .map_err(|error| {
            AsterError::config_error(format!(
                "failed to reach master enrollment ack endpoint: {error}"
            ))
        })?;

    parse_empty_api_response(response, "master enrollment ack request").await
}

async fn parse_api_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    action: &str,
) -> Result<T> {
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        AsterError::config_error(format!("failed to read {action} response body: {error}"))
    })?;
    let envelope: ApiEnvelope<T> = serde_json::from_slice(&body).map_err(|error| {
        AsterError::config_error(format!("failed to parse {action} response: {error}"))
    })?;

    if !status.is_success() || envelope.code != 0 {
        let message = if envelope.msg.trim().is_empty() {
            format!("{action} failed with HTTP {status}")
        } else {
            envelope.msg
        };
        return Err(AsterError::validation_error(message));
    }

    envelope
        .data
        .ok_or_else(|| AsterError::config_error(format!("{action} response is missing data")))
}

async fn parse_empty_api_response(response: reqwest::Response, action: &str) -> Result<()> {
    let status = response.status();
    let body = response.bytes().await.map_err(|error| {
        AsterError::config_error(format!("failed to read {action} response body: {error}"))
    })?;
    let envelope: ApiEnvelope<serde_json::Value> =
        serde_json::from_slice(&body).map_err(|error| {
            AsterError::config_error(format!("failed to parse {action} response: {error}"))
        })?;

    if !status.is_success() || envelope.code != 0 {
        let message = if envelope.msg.trim().is_empty() {
            format!("{action} failed with HTTP {status}")
        } else {
            envelope.msg
        };
        return Err(AsterError::validation_error(message));
    }

    Ok(())
}

async fn resolve_ingress_policy(
    db: &sea_orm::DatabaseConnection,
    ingress_policy_id: Option<i64>,
) -> Result<crate::entities::storage_policy::Model> {
    let policy = if let Some(policy_id) = ingress_policy_id {
        policy_repo::find_by_id(db, policy_id).await?
    } else {
        policy_repo::find_default(db).await?.ok_or_else(|| {
            AsterError::storage_policy_not_found(
                "no default storage policy found; pass --ingress-policy-id explicitly",
            )
        })?
    };

    if policy.driver_type == DriverType::Remote {
        return Err(AsterError::validation_error(
            "ingress policy cannot use the remote driver",
        ));
    }

    Ok(policy)
}

fn render_node_human(report: &NodeEnrollReport) -> String {
    let palette = CliTerminalPalette::stdout();
    let title = palette.title("AsterDrive Node Enrollment");
    let status = palette.status_badge("ok");
    let mut lines = vec![
        format!("{title} {status}"),
        palette.dim("--------------------------------------------------"),
        format!("{}{}", human_key("Action", &palette), report.action),
        format!(
            "{}{} (#{} )",
            human_key("Binding", &palette),
            report.binding_name,
            report.binding_id
        )
        .replace("#", "#")
        .replace(" )", ")"),
        format!("{}{}", human_key("Master URL", &palette), report.master_url),
        format!("{}{}", human_key("Namespace", &palette), report.namespace),
        format!("{}{}", human_key("Access Key", &palette), report.access_key),
        format!(
            "{}{} (#{} )",
            human_key("Ingress", &palette),
            report.ingress_policy_name,
            report.ingress_policy_id
        )
        .replace("#", "#")
        .replace(" )", ")"),
        format!("{}{}", human_key("Config", &palette), report.config_path),
        format!(
            "{}{}:{}",
            human_key("Listen", &palette),
            report.server_host,
            report.server_port
        ),
    ];

    lines.push(String::new());
    lines.push(palette.label("Next steps:"));
    lines.push("  1. Restart the AsterDrive process on this node so the follower endpoint starts listening.".to_string());
    lines.push(format!(
        "  2. Confirm the master can reach this node on {}:{}.",
        palette.accent(&report.server_host),
        report.server_port
    ));
    lines.push(format!("     {}", report.connectivity_hint));
    lines.push(format!(
        "  3. Verify {} through the same address the master will use.",
        palette.accent(&report.readiness_check_path)
    ));

    lines.join("\n")
}

fn build_connectivity_hint(server_host: &str, server_port: u16) -> String {
    if host_is_loopback(server_host) {
        return format!(
            "Current server.host is {server_host}. If the master runs on another machine, change server.host or put a reverse proxy/tunnel in front of port {server_port}."
        );
    }

    format!(
        "If you publish the follower through a reverse proxy, NAT, or port mapping, make sure it still forwards to port {server_port}."
    )
}

fn host_is_loopback(server_host: &str) -> bool {
    let trimmed = server_host.trim();
    trimmed.eq_ignore_ascii_case("localhost")
        || trimmed
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::{
        FOLLOWER_DEFAULT_SERVER_HOST, NodeEnrollReport, follower_seed_config, host_is_loopback,
        render_node_human,
    };

    #[test]
    fn render_node_human_focuses_on_connectivity_steps() {
        let report = NodeEnrollReport {
            action: "created",
            binding_id: 7,
            binding_name: "node-a".to_string(),
            master_url: "http://localhost:3000".to_string(),
            namespace: "team-alpha".to_string(),
            access_key: "ak_test".to_string(),
            ingress_policy_id: 3,
            ingress_policy_name: "Local Default".to_string(),
            config_path: "data/config.toml".to_string(),
            server_host: "127.0.0.1".to_string(),
            server_port: 3000,
            readiness_check_path: "/health/ready".to_string(),
            connectivity_hint:
                "Current server.host is 127.0.0.1. If the master runs on another machine, change server.host or put a reverse proxy/tunnel in front of port 3000."
                    .to_string(),
        };

        let rendered = render_node_human(&report);

        assert!(rendered.contains("Next steps:"));
        assert!(rendered.contains("Listen"));
        assert!(rendered.contains("127.0.0.1:3000"));
        assert!(rendered.contains("Confirm the master can reach this node"));
        assert!(rendered.contains("/health/ready"));
    }

    #[test]
    fn host_is_loopback_detects_local_hosts() {
        assert!(host_is_loopback("127.0.0.1"));
        assert!(host_is_loopback("::1"));
        assert!(host_is_loopback("localhost"));
        assert!(!host_is_loopback("0.0.0.0"));
        assert!(!host_is_loopback("192.168.1.10"));
    }

    #[test]
    fn follower_seed_config_uses_public_bind_host() {
        let config = follower_seed_config();

        assert_eq!(config.server.start_mode, super::NodeRuntimeMode::Follower);
        assert_eq!(config.server.host, FOLLOWER_DEFAULT_SERVER_HOST);
    }
}
