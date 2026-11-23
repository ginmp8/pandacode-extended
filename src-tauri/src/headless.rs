//! Headless host mode: `pandacode serve` (docs/headless.md) runs the desktop runtime with
//! no window and no webview on an always-on server, hosting PandaCode Connect clients —
//! the mobile app and other computers' PandaCode see it as a computer and can start,
//! watch, steer and approve tasks on it. The window's few real jobs for a server (queue
//! drain, pull-request links, limit resumes) run natively here instead; see start_services.

use crate::{message_queue, AppState};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

static SERVE: AtomicBool = AtomicBool::new(false);

/// True inside a `pandacode serve` process. Native hooks key off this instead of a window.
pub(crate) fn active() -> bool {
    SERVE.load(Ordering::Relaxed)
}

/// What this `serve` invocation does. One per process.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Serve,
    SignIn { name: Option<String> },
    SignOut,
    FullAccess { enabled: bool },
    ExportBots { path: PathBuf },
    SyncBots { path: PathBuf },
    PreviewOrchestrations { path: PathBuf },
    ImportOrchestrations { path: PathBuf },
    ExportOrchestrations { path: PathBuf },
    InspectOrchestrations,
    CreateOrchestration { config_file: PathBuf },
    DuplicateOrchestration { source_id: String, expected_revision: i64, name: String },
    ListTickets,
    ListTicketsPage { before_created_at: Option<i64>, before_id: Option<String> },
    ListTicketActivity { ticket_id: String, before_id: Option<i64> },
    ListTeamMemberActivity { orchestration_id: String, bot_id: String,
        before_created_at: Option<i64>, before_assignment_id: Option<String> },
    ListReviewInbox { before_updated_at: Option<i64>, before_id: Option<String> },
    ListTeamReviewInbox { orchestration_id: String, before_updated_at: Option<i64>, before_id: Option<String> },
    ListRecoveryInbox { before_priority: Option<i64>, before_updated_at: Option<i64>, before_id: Option<String> },
    ListTeamRecoveryInbox { orchestration_id: String, before_priority: Option<i64>, before_updated_at: Option<i64>, before_id: Option<String> },
    ListUnresolvedEffects { before_updated_at: Option<i64>, before_ticket_id: Option<String>, before_effect_key: Option<String> },
    ListTeamUnresolvedEffects { orchestration_id: String, before_updated_at: Option<i64>, before_ticket_id: Option<String>, before_effect_key: Option<String> },
    ListDemands {
        state: String, before_updated_at: Option<i64>, before_id: Option<String>,
        search_query: Option<String>, project_id: Option<String>,
    },
    ListTeamDemands {
        orchestration_id: String, state: String,
        before_updated_at: Option<i64>, before_id: Option<String>,
        search_query: Option<String>, project_id: Option<String>,
    },
    InspectTicket { ticket_id: String },
    InspectTicketGraph { ticket_id: String },
    InspectTicketEffects { ticket_id: String, before_created_at: Option<i64>, before_key: Option<String>, unresolved_only: bool },
    InspectTicketSource {
        ticket_id: String, paginated: bool, before_at: Option<i64>,
        before_source_id: Option<String>, before_delivery_id: Option<String>,
    },
    ListTicketSourceLinks { ticket_id: String },
    InspectSourceHealth { source_id: String },
    InspectSourceDecisions {
        source_id: String, paginated: bool,
        before_sequence: Option<i64>, before_delivery_id: Option<String>,
    },
    LinkSourceThread { source_id: String, ticket_id: String, subject_key: String },
    LinkIssuePull { source_id: String, ticket_id: String, issue_key: String, pull_key: String },
    VerifyLinkIssuePull { source_id: String, ticket_id: String, issue_key: String, pull_key: String },
    UnlinkSourceThread { source_id: String, ticket_id: String, subject_key: String },
    InspectTeamReadiness { orchestration_id: String },
    PreflightDemand { orchestration_id: String, coordinated: bool, objective_file: PathBuf },
    PreflightWorkerDemand { orchestration_id: String, worker_bot_id: String, objective_file: PathBuf },
    InspectTeamRoster { orchestration_id: String },
    CreateSource { orchestration_id: String, repository: String, actor_filter: String, event_kind: String },
    ListTeamSources { orchestration_id: String },
    SetSourceMode { source_id: String, expected_revision: i64, mode: String },
    ObserveSource { source_id: String },
    ObserveTeamSources { orchestration_id: String },
    ObserveCheckRun { source_id: String, check_run_id: i64 },
    ObserveLinkedPrChecks { source_id: String, ticket_id: String, pull_key: String },
    QueueDemand { orchestration_id: String, request_key: String, objective_file: PathBuf },
    QueueSelectedWorkerDemand { orchestration_id: String, worker_bot_id: String, request_key: String, objective_file: PathBuf },
    AcceptTicket { ticket_id: String, expected_revision: i64 },
    RejectTicket { ticket_id: String, expected_revision: i64, reason: String },
    StopTicket { ticket_id: String },
    StopTicketAtRevision { ticket_id: String, expected_revision: i64 },
    QuarantineDetachedTicket { ticket_id: String, expected_revision: i64 },
    QuarantineExpiredTicket { ticket_id: String, expected_revision: i64 },
    QuarantinePendingEffect { ticket_id: String, expected_revision: i64, effect_key: String, payload_sha256: String },
    QuarantineOrphanRunning { ticket_id: String, expected_revision: i64 },
    QuarantineTerminalOrphan { ticket_id: String, expected_revision: i64 },
    DisableOrchestration { orchestration_id: String, expected_revision: i64 },
    EnableOrchestration { orchestration_id: String, expected_revision: i64 },
    RenameOrchestration { orchestration_id: String, expected_revision: i64, name: String },
    SetTeamStrategy { orchestration_id: String, expected_revision: i64, strategy: String, purpose_file: PathBuf },
    SetTeamRoster { orchestration_id: String, expected_revision: i64, roster_file: PathBuf },
    SetTeamAttemptLimit { orchestration_id: String, expected_revision: i64, limit: i64 },
}

fn read_bounded_objective(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Cannot read objective file: {e}"))?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1).read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read bounded objective: {e}"))?;
    if bytes.len() > 16 * 1024 {
        return Err("Objective exceeds the native 16 KiB limit.".into());
    }
    let objective = std::str::from_utf8(&bytes)
        .map_err(|_| "Objective file must contain UTF-8 text.")?;
    Ok(objective.to_owned())
}

/// Read a complete, bounded local Team role-selection document. Bot identifiers
/// are only local references: this never installs a bot or grants execution.
/// Reuse the existing 16 KiB UTF-8 limit and redact malformed file contents.
fn read_bounded_roster(path: &std::path::Path)
    -> Result<crate::superagent::roster::RosterInput, String>
{
    let text = read_bounded_objective(path)
        .map_err(|_| "Cannot read bounded UTF-8 Team roster file.".to_owned())?;
    serde_json::from_str(&text)
        .map_err(|_| "Team roster file must contain workerBotIds and reviewerBotIds arrays.".into())
}


#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeadlessTeamCreateDocument {
    config: crate::superagent::NewOrchestration,
    roster: crate::superagent::roster::RosterInput,
    max_worker_attempts: Option<i64>,
    purpose: Option<String>,
    strategy: Option<String>,
}

/// Configuration-only local JSON. Unknown authority fields, oversized files
/// and malformed role documents fail before any native Team transaction.
fn read_bounded_team_creation(path: &std::path::Path) -> Result<HeadlessTeamCreateDocument, String> {
    let content = read_bounded_objective(path)
        .map_err(|_| "Cannot read bounded UTF-8 Team creation file.".to_owned())?;
    serde_json::from_str(&content)
        .map_err(|_| "Invalid Team configuration/roster JSON or unsupported authority field.".into())
}

/// `pandacode serve …`: true when the argument was handled; the process then serves until
/// it is stopped (or exits after a settings command). Everything else is the desktop app.
pub fn handle() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("serve") {
        return false;
    }
    match parse(&args[1..]) {
        Ok((command, import)) => {
            SERVE.store(true, Ordering::Relaxed);
            // A server has no Secret Service agent: the 0600 file backend is serve's default.
            // Set PANDACODE_SECRETS_FILE=0 to use the OS store anyway.
            if std::env::var_os("PANDACODE_SECRETS_FILE").is_none() {
                std::env::set_var("PANDACODE_SECRETS_FILE", "1");
            }
            serve(command, import)
        }
        Err(help) => {
            eprintln!("{help}");
            std::process::exit(2)
        }
    }
}

/// Builds the app with no window and runs it until an explicit exit. Never returns.
fn serve(command: Command, import: Option<PathBuf>) -> ! {
    let app = tauri::Builder::default()
        .setup(move |app| {
            // Portable manifest operations are also detached administrative
            // commands. Store::open inside setup_common would revoke healthy
            // serve-owned Tickets and quarantine pending effects on startup.
            if matches!(&command,
                Command::PreviewOrchestrations { .. }
                | Command::ImportOrchestrations { .. }
                | Command::ExportOrchestrations { .. }
            ) {
                let result = (|| -> Result<String, String> {
                    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                    let database = dir.join("kingcode.sqlite3");
                    match &command {
                        Command::PreviewOrchestrations { path } => {
                            let plan = crate::superagent::manifest::run_headless_preview(
                                &database, path,
                            )?;
                            Ok(format!(
                                "Would create disabled presets: {}\nUnchanged presets: {}",
                                plan.would_create.join(", "), plan.unchanged.join(", "),
                            ))
                        }
                        Command::ImportOrchestrations { path } => {
                            let imported = crate::superagent::manifest::run_headless_import(
                                &database, &dir, path,
                            )?;
                            let mut summary = format!(
                                "Created {} disabled orchestration presets; {} unchanged.",
                                imported.created.len(), imported.unchanged.len(),
                            );
                            if let Some(backup) = imported.backup_path {
                                summary.push_str(&format!("\nVerified orchestration backup: {backup}"));
                            }
                            Ok(summary)
                        }
                        Command::ExportOrchestrations { path } => {
                            crate::superagent::manifest::run_headless_export(&database, path)?;
                            Ok(format!(
                                "Orchestration configuration written to {}.", path.display(),
                            ))
                        }
                        _ => unreachable!(),
                    }
                })();
                match result {
                    Ok(summary) => { println!("{summary}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent manifest operation failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            // One-shot inspection and Ticket decisions precede setup_common:
            // Store::open performs restart recovery and could interrupt a
            // healthy concurrently running serve process.
            let inspect = match &command {
                Command::InspectOrchestrations => Some(None),
                Command::InspectTicket { ticket_id } => Some(Some(ticket_id.as_str())),
                _ => None,
            };
            if let Some(ticket_id) = inspect {
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::inspect_headless_status(
                        &dir.join("kingcode.sqlite3"), ticket_id,
                    ));
                match result {
                    Ok(status) => { println!("{status}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::InspectTicketGraph { ticket_id } = &command {
                // The same durable, bounded native Ticket/Assignment graph as the
                // workspace. Do not run Store::open recovery beside a live host.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::inspect_headless_ticket_graph(
                        &dir.join("kingcode.sqlite3"), ticket_id,
                    ));
                match result {
                    Ok(graph) => { println!("{graph}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent graph inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::InspectTicketEffects { ticket_id, before_created_at, before_key, unresolved_only } = &command {
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let database = dir.join("kingcode.sqlite3");
                        if before_created_at.is_none() && !unresolved_only {
                            crate::superagent::inspect_headless_effect_receipts(&database, ticket_id)
                        } else {
                            crate::superagent::inspect_headless_effect_receipts_page(
                                &database, ticket_id, *before_created_at,
                                before_key.as_deref(), *unresolved_only,
                            )
                        }
                    });
                match result {
                    Ok(receipts) => { println!("{receipts}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent effect receipt inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::InspectTicketSource {
                ticket_id, paginated, before_at, before_source_id, before_delivery_id,
            } = &command {
                // No GitHub network, grant, provider launch or startup recovery.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let database = dir.join("kingcode.sqlite3");
                        if *paginated {
                            let cursor = before_at.map(|at|
                                crate::superagent::events::LinkedActivityCursor {
                                    at, source_id:before_source_id.clone().unwrap_or_default(),
                                    delivery_id:before_delivery_id.clone().unwrap_or_default(),
                                });
                            crate::superagent::events::headless_linked_activity_page(
                                &database, ticket_id, 50, cursor.as_ref(),
                            )
                        } else {
                            crate::superagent::events::headless_linked_activity(
                                &database, ticket_id, 50,
                            )
                        }
                    });
                match result {
                    Ok(activity) => { println!("{activity}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent source activity inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTicketSourceLinks { ticket_id } = &command {
                // The desktop and detached CLI read the same exact native links.
                // This never polls GitHub, authorizes a task or runs restart recovery.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_ticket_thread_links(
                        &dir.join("kingcode.sqlite3"), ticket_id,
                    ));
                match result {
                    Ok(links) => { println!("{links}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent source thread link inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::InspectSourceHealth { source_id } = &command {
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_inspect_source_integrity(
                        &dir.join("kingcode.sqlite3"), source_id,
                    ));
                match result {
                    Ok(status) => { println!("{status}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent source health inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::InspectSourceDecisions {
                source_id, paginated, before_sequence, before_delivery_id,
            } = &command {
                // Immutable source-scoped receipts, no GitHub polling, provider
                // admission, task grant or startup recovery.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let database = dir.join("kingcode.sqlite3");
                        if *paginated {
                            crate::superagent::events::headless_source_decisions_page(
                                &database, source_id, 50,
                                *before_sequence, before_delivery_id.as_deref(),
                            )
                        } else {
                            crate::superagent::events::headless_source_decisions(
                                &database, source_id, 50,
                            )
                        }
                    });
                match result {
                    Ok(history) => { println!("{history}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent source decision inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::LinkSourceThread { source_id, ticket_id, subject_key } = &command {
                // Local context only; reuse the desktop native transaction without
                // startup recovery, provider launch or inferred GitHub authority.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_link_thread(
                        &dir.join("kingcode.sqlite3"), source_id, ticket_id, subject_key,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent thread link failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::LinkIssuePull { source_id, ticket_id, issue_key, pull_key } = &command {
                // Reuse Desktop's single native association/audit transaction,
                // with no provider, GitHub write or inferred task authority.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_link_issue_pull(
                        &dir.join("kingcode.sqlite3"), source_id, ticket_id, issue_key, pull_key,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Issue/PR association failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::VerifyLinkIssuePull { source_id, ticket_id, issue_key, pull_key } = &command {
                // Local authenticated read-only GitHub API lookup, followed
                // by a guarded native Issue/PR context-link transaction.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_verify_link_issue_pull(
                        &dir.join("kingcode.sqlite3"), source_id, ticket_id, issue_key, pull_key,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent GitHub Issue/PR verification failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::UnlinkSourceThread { source_id, ticket_id, subject_key } = &command {
                // One explicit trusted local mutation, no Store::open recovery,
                // provider launch, grant, GitHub write, or implicit refollow.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_unlink_thread(
                        &dir.join("kingcode.sqlite3"), source_id, ticket_id, subject_key,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent thread unlink failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::PreflightDemand { orchestration_id, coordinated, objective_file } = &command {
                // An exact objective preflight cannot issue a Mandate or run a provider.
                // The detached CLI cannot attest live session occupancy; actual
                // admission must repeat the entire native policy and lease check.
                let result = (|| -> Result<String, String> {
                    let objective = read_bounded_objective(objective_file)?;
                    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                    crate::superagent::inspect_headless_demand_preflight(
                        &dir.join("kingcode.sqlite3"), orchestration_id, &objective, *coordinated,
                    )
                })();
                match result {
                    Ok(preflight) => { println!("{preflight}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Demand preflight failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::PreflightWorkerDemand {
                orchestration_id, worker_bot_id, objective_file,
            } = &command {
                // Read-only explicit member preflight; not an admission or a grant.
                // The live host must recheck occupancy and native policy later.
                let result = (|| -> Result<String, String> {
                    let objective = read_bounded_objective(objective_file)?;
                    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                    crate::superagent::inspect_headless_worker_demand_preflight(
                        &dir.join("kingcode.sqlite3"), orchestration_id, worker_bot_id, &objective,
                    )
                })();
                match result {
                    Ok(preflight) => { println!("{preflight}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Worker Demand preflight failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::InspectTeamReadiness { orchestration_id } = &command {
                // Read-only on an existing database; do not run startup recovery
                // against another live headless process. This is not admission.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::inspect_headless_team_readiness(
                        &dir.join("kingcode.sqlite3"), orchestration_id,
                    ));
                match result {
                    Ok(readiness) => { println!("{readiness}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team readiness inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::InspectTeamRoster { orchestration_id } = &command {
                let result=app.path().app_data_dir()
                    .map_err(|e|e.to_string())
                    .and_then(|dir| crate::superagent::inspect_headless_team_roster(
                        &dir.join("kingcode.sqlite3"),orchestration_id,
                    ));
                match result {
                    Ok(roster) => { println!("{roster}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team roster inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if matches!(&command, Command::ListTickets) {
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_tasks(
                        &dir.join("kingcode.sqlite3"),
                    ));
                match result {
                    Ok(tasks) => { println!("{tasks}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent task listing failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTicketsPage { before_created_at, before_id } = &command {
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_tasks_page(
                        &dir.join("kingcode.sqlite3"), *before_created_at, before_id.as_deref(),
                    ));
                match result {
                    Ok(tasks) => { println!("{tasks}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent paged Ticket listing failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTicketActivity { ticket_id, before_id } = &command {
                // Read only the current Ticket's redacted audit metadata. Never
                // open a recovering store or reveal private event details.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_ticket_activity(
                        &dir.join("kingcode.sqlite3"), ticket_id, *before_id,
                    ));
                match result {
                    Ok(activity) => { println!("{activity}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Ticket activity inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTeamMemberActivity {
                orchestration_id, bot_id, before_created_at, before_assignment_id,
            } = &command {
                // Inspect native activity without opening a recovering store.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_team_member_activity(
                        &dir.join("kingcode.sqlite3"), orchestration_id, bot_id,
                        *before_created_at, before_assignment_id.as_deref(),
                    ));
                match result {
                    Ok(activity) => { println!("{activity}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team member activity inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListRecoveryInbox { before_priority,before_updated_at,before_id } = &command {
                // Recovery planning is read-only even beside a live serve host:
                // no restart settlement, provider replay, or effect acknowledgment.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let database = dir.join("kingcode.sqlite3");
                        if before_priority.is_none() && before_updated_at.is_none()
                            && before_id.is_none()
                        {
                            crate::superagent::list_headless_recovery_inbox(&database)
                        } else {
                            crate::superagent::list_headless_recovery_inbox_page(
                                &database, *before_priority, *before_updated_at,
                                before_id.as_deref(),
                            )
                        }
                    });
                match result {
                    Ok(inbox) => { println!("{inbox}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent recovery inbox inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTeamRecoveryInbox {
                orchestration_id, before_priority, before_updated_at, before_id,
            } = &command {
                // Scope native recovery candidates and cursor anchors in SQL before
                // paging. This is read-only, including beside a healthy serve host.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_team_recovery_inbox_page(
                        &dir.join("kingcode.sqlite3"), orchestration_id,
                        *before_priority, *before_updated_at, before_id.as_deref(),
                    ));
                match result {
                    Ok(inbox) => { println!("{inbox}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team recovery inbox inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListUnresolvedEffects { before_updated_at, before_ticket_id, before_effect_key } = &command {
                // Cross-Ticket native receipt inventory; no grant, replay, provider
                // contact or startup recovery against another live serve host.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_unresolved_effects(
                        &dir.join("kingcode.sqlite3"), *before_updated_at,
                        before_ticket_id.as_deref(), before_effect_key.as_deref(),
                    ));
                match result {
                    Ok(inbox) => { println!("{inbox}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent effect reconciliation inbox failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTeamUnresolvedEffects {
                orchestration_id, before_updated_at, before_ticket_id, before_effect_key,
            } = &command {
                // Native Team filter applies before pagination, without provider or
                // startup recovery; a detached operator cannot authorize replay.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_team_unresolved_effects(
                        &dir.join("kingcode.sqlite3"), orchestration_id, *before_updated_at,
                        before_ticket_id.as_deref(), before_effect_key.as_deref(),
                    ));
                match result {
                    Ok(inbox) => { println!("{inbox}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team effect reconciliation inbox failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListReviewInbox { before_updated_at, before_id } = &command {
                // Native pending-decision cursor; no startup recovery or grants.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let database = dir.join("kingcode.sqlite3");
                        if before_updated_at.is_none() {
                            crate::superagent::list_headless_review_inbox(&database)
                        } else {
                            crate::superagent::list_headless_review_inbox_page(
                                &database, *before_updated_at, before_id.as_deref(),
                            )
                        }
                    });
                match result {
                    Ok(inbox) => { println!("{inbox}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent review inbox inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTeamReviewInbox {
                orchestration_id, before_updated_at, before_id,
            } = &command {
                // Detached native review observation never performs grants or approval.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::list_headless_team_review_inbox_page(
                        &dir.join("kingcode.sqlite3"), orchestration_id,
                        *before_updated_at, before_id.as_deref(),
                    ));
                match result {
                    Ok(inbox) => { println!("{inbox}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team review inbox inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListDemands {
                state, before_updated_at, before_id, search_query, project_id,
            } = &command {
                // Read-only global queue, no startup recovery or task dispatch.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let db = dir.join("kingcode.sqlite3");
                        if let Some(project) = project_id.as_deref() {
                            crate::superagent::list_headless_all_demands_project_searched(
                                &db, state, *before_updated_at, before_id.as_deref(),
                                search_query.as_deref(), Some(project),
                            )
                        } else {
                            crate::superagent::list_headless_all_demands_searched(
                                &db, state, *before_updated_at, before_id.as_deref(),
                                search_query.as_deref(),
                            )
                        }
                    });
                match result {
                    Ok(page) => { println!("{page}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Demand queue inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTeamDemands {
                orchestration_id, state, before_updated_at, before_id, search_query, project_id,
            } = &command {
                // Read the same source-of-truth Ticket state index as desktop,
                // without mutating a concurrent live serve process.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let db = dir.join("kingcode.sqlite3");
                        if let Some(project) = project_id.as_deref() {
                            crate::superagent::list_headless_team_demands_project_searched(
                                &db, orchestration_id, state, *before_updated_at,
                                before_id.as_deref(), search_query.as_deref(), Some(project),
                            )
                        } else if let Some(query) = search_query.as_deref() {
                            crate::superagent::list_headless_team_demands_searched(
                                &db, orchestration_id, state, *before_updated_at,
                                before_id.as_deref(), Some(query),
                            )
                        } else {
                            crate::superagent::list_headless_team_demands(
                                &db, orchestration_id, state, *before_updated_at,
                                before_id.as_deref(),
                            )
                        }
                    });
                match result {
                    Ok(page) => { println!("{page}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team Demand inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::CreateSource {
                orchestration_id, repository, actor_filter, event_kind,
            } = &command {
                // Operator-created, disabled-by-default native source. Does not
                // authenticate events, launch a bot or recover a live serve host.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_create_source(
                        &dir.join("kingcode.sqlite3"), orchestration_id,
                        repository, actor_filter, event_kind,
                    ));
                match result {
                    Ok(source) => { println!("{source}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent source creation failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ListTeamSources { orchestration_id } = &command {
                // Read only: preserve concurrent server ownership and
                // never run startup recovery to inspect Team source policy.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_list_team_sources(
                        &dir.join("kingcode.sqlite3"), orchestration_id,
                    ));
                match result {
                    Ok(sources) => { println!("{sources}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team source inspection failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::SetSourceMode { source_id, expected_revision, mode } = &command {
                // Trusted local operator action; the native source policy CAS
                // is shared with desktop and revokes Observe immediately.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_set_source_mode(
                        &dir.join("kingcode.sqlite3"), source_id, *expected_revision, mode,
                    ));
                match result {
                    Ok(source) => { println!("{source}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent source policy change failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ObserveSource { source_id } = &command {
                // The same authenticated Events adapter and policy-revision
                // transaction as desktop, without Webview or startup recovery.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_observe_source(
                        &dir.join("kingcode.sqlite3"), source_id,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent source observation failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ObserveTeamSources { orchestration_id } = &command {
                // Operator-invoked, bounded multi-source Observe only. A failure
                // is nonzero; earlier source receipts stay durable/idempotent.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_observe_team_sources(
                        &dir.join("kingcode.sqlite3"), orchestration_id,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team source observation failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ObserveCheckRun { source_id, check_run_id } = &command {
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_observe_check_run(
                        &dir.join("kingcode.sqlite3"), source_id, *check_run_id,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Check Run observation failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::ObserveLinkedPrChecks { source_id, ticket_id, pull_key } = &command {
                // Exact operator-linked PR and authenticated Checks only;
                // no WorkItem, Council pass, provider admission or remote write.
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| crate::superagent::events::headless_observe_linked_pr_checks(
                        &dir.join("kingcode.sqlite3"), source_id, ticket_id, pull_key,
                    ));
                match result {
                    Ok(receipt) => { println!("{receipt}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent linked PR Checks observation failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::QueueDemand { orchestration_id, request_key, objective_file } = &command {
                // The objective is a bounded local file, never a command-line
                // argument or console payload. This command stages but does not run.
                let result = (|| -> Result<String, String> {
                    let objective = read_bounded_objective(objective_file)?;
                    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                    crate::superagent::headless_queue_demand(
                        &dir.join("kingcode.sqlite3"), orchestration_id, &objective, request_key,
                    )
                })();
                match result {
                    Ok(status) => { println!("{status}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Demand staging failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            if let Command::QueueSelectedWorkerDemand {
                orchestration_id, worker_bot_id, request_key, objective_file,
            } = &command {
                // A named secondary Worker is pinned by native Ticket admission,
                // but this detached command must never start a provider session.
                let result = (|| -> Result<String, String> {
                    let objective = read_bounded_objective(objective_file)?;
                    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                    crate::superagent::headless_queue_selected_worker_demand(
                        &dir.join("kingcode.sqlite3"), orchestration_id, worker_bot_id,
                        &objective, request_key,
                    )
                })();
                match result {
                    Ok(status) => { println!("{status}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent selected Worker Demand staging failed: {error}");
                        std::process::exit(1)
                    }
                }
            }

            if let Command::CreateOrchestration { config_file } = &command {
                // Create a disabled Team through the same atomic native policy
                // transaction as desktop. The local document is not an
                // execution, provider or GitHub authorization.
                let result = (|| -> Result<String, String> {
                    let config = read_bounded_team_creation(config_file)?;
                    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                    crate::superagent::headless_create_orchestration(
                        &dir.join("kingcode.sqlite3"), config.config, config.roster,
                        config.max_worker_attempts.unwrap_or(2),
                        config.purpose.as_deref().unwrap_or(""),
                        config.strategy.as_deref().unwrap_or("balanced"),
                    )
                })();
                match result {
                    Ok(status) => { println!("{status}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent Team creation failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            let native_action = matches!(&command,
                Command::AcceptTicket { .. } | Command::RejectTicket { .. }
                | Command::StopTicket { .. } | Command::StopTicketAtRevision { .. }
                | Command::QuarantineDetachedTicket { .. }
                | Command::QuarantineExpiredTicket { .. }
                | Command::QuarantinePendingEffect { .. }
                | Command::QuarantineOrphanRunning { .. }
                | Command::QuarantineTerminalOrphan { .. }
                | Command::DisableOrchestration { .. }
                | Command::EnableOrchestration { .. }
                | Command::RenameOrchestration { .. }
                | Command::DuplicateOrchestration { .. }
                | Command::SetTeamStrategy { .. }
                | Command::SetTeamRoster { .. }
                | Command::SetTeamAttemptLimit { .. });
            if native_action {
                let result = app.path().app_data_dir()
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        let database = dir.join("kingcode.sqlite3");
                        match &command {
                            Command::AcceptTicket { ticket_id, expected_revision } =>
                                crate::superagent::headless_accept_ticket(&database, ticket_id, *expected_revision),
                            Command::RejectTicket { ticket_id, expected_revision, reason } =>
                                crate::superagent::headless_reject_ticket(&database, ticket_id, *expected_revision, reason),
                            Command::StopTicket { ticket_id } =>
                                crate::superagent::headless_stop_ticket(&database, ticket_id),
                            Command::StopTicketAtRevision { ticket_id, expected_revision } =>
                                crate::superagent::headless_stop_ticket_at_revision(
                                    &database, ticket_id, *expected_revision,
                                ),
                            Command::QuarantineDetachedTicket { ticket_id, expected_revision } =>
                                crate::superagent::headless_quarantine_detached_ticket(
                                    &database,ticket_id,*expected_revision),
                            Command::QuarantineExpiredTicket { ticket_id, expected_revision } =>
                                crate::superagent::headless_quarantine_expired_ticket(
                                    &database,ticket_id,*expected_revision),
                            Command::QuarantinePendingEffect { ticket_id, expected_revision, effect_key, payload_sha256 } =>
                                crate::superagent::headless_quarantine_pending_effect(
                                    &database, ticket_id, *expected_revision, effect_key, payload_sha256),
                            Command::QuarantineOrphanRunning { ticket_id, expected_revision } =>
                                crate::superagent::headless_quarantine_orphan_running_ticket(
                                    &database,ticket_id,*expected_revision),
                            Command::QuarantineTerminalOrphan { ticket_id, expected_revision } =>
                                crate::superagent::headless_quarantine_terminal_orphan(
                                    &database, ticket_id, *expected_revision),
                            Command::DisableOrchestration { orchestration_id, expected_revision } =>
                                crate::superagent::headless_disable_orchestration(
                                    &database, orchestration_id, *expected_revision,
                                ),
                            Command::EnableOrchestration { orchestration_id, expected_revision } =>
                                crate::superagent::headless_enable_orchestration(
                                    &database, orchestration_id, *expected_revision,
                                ),
                            Command::DuplicateOrchestration { source_id, expected_revision, name } =>
                                crate::superagent::headless_duplicate_orchestration(
                                    &database, source_id, *expected_revision, name,
                                ),
                            Command::RenameOrchestration { orchestration_id, expected_revision, name } =>
                                crate::superagent::headless_rename_orchestration(
                                    &database, orchestration_id, *expected_revision, name,
                                ),
                            Command::SetTeamRoster { orchestration_id, expected_revision, roster_file } => {
                                let members = read_bounded_roster(roster_file)?;
                                crate::superagent::headless_set_team_roster(
                                    &database, orchestration_id, *expected_revision, &members,
                                )
                            }
                            Command::SetTeamAttemptLimit { orchestration_id, expected_revision, limit } =>
                                crate::superagent::headless_set_team_attempt_limit(
                                    &database, orchestration_id, *expected_revision, *limit,
                                ),
                            Command::SetTeamStrategy { orchestration_id, expected_revision, strategy, purpose_file } => {
                                let purpose = read_bounded_objective(purpose_file)?;
                                crate::superagent::headless_set_team_strategy(
                                    &database, orchestration_id, *expected_revision, &purpose, strategy,
                                )
                            }
                            _ => unreachable!(),
                        }
                    });
                match result {
                    Ok(status) => { println!("{status}"); std::process::exit(0) }
                    Err(error) => {
                        eprintln!("SuperAgent native command failed: {error}");
                        std::process::exit(1)
                    }
                }
            }
            let dir = crate::setup_common(app)?;
            // No window on a server: close the one from tauri.conf.json right away; the
            // run loop keeps the process alive (see on_run_event). Users watch from their
            // devices, never from here.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
                let _ = window.close();
            }
            start(app.handle(), &dir, command, import);
            Ok(())
        })
        .build(crate::app_context())
        .expect("Failed to launch PandaCode");
    app.run(crate::on_run_event);
    std::process::exit(0)
}

/// Per-command startup, run from setup.
fn start(app: &tauri::AppHandle, dir: &std::path::Path, command: Command, import: Option<PathBuf>) {
    match command {
        // Inspection exits before setup_common and must never reach this branch.
        Command::PreviewOrchestrations { .. }
        | Command::ImportOrchestrations { .. }
        | Command::ExportOrchestrations { .. }
        | Command::InspectOrchestrations | Command::CreateOrchestration { .. }
        | Command::ListTickets | Command::ListTicketsPage { .. } | Command::ListTicketActivity { .. }
        | Command::ListTeamMemberActivity { .. } | Command::ListReviewInbox { .. } | Command::ListTeamReviewInbox { .. }
        | Command::ListRecoveryInbox { .. } | Command::ListTeamRecoveryInbox { .. }
        | Command::ListUnresolvedEffects { .. }
        | Command::ListTeamUnresolvedEffects { .. } | Command::ListDemands { .. }
        | Command::ListTeamDemands { .. }
        | Command::InspectTicket { .. } | Command::InspectTicketGraph { .. }
        | Command::InspectTeamReadiness { .. } | Command::InspectTeamRoster { .. }
        | Command::PreflightDemand { .. } | Command::PreflightWorkerDemand { .. }
        | Command::InspectTicketEffects { .. }
        | Command::InspectTicketSource { .. } | Command::ListTicketSourceLinks { .. }
        | Command::InspectSourceHealth { .. }
        | Command::InspectSourceDecisions { .. }
        | Command::LinkSourceThread { .. } | Command::LinkIssuePull { .. } | Command::VerifyLinkIssuePull { .. }
        | Command::UnlinkSourceThread { .. }
        | Command::CreateSource { .. } | Command::ListTeamSources { .. }
        | Command::SetSourceMode { .. }
        | Command::ObserveSource { .. } | Command::ObserveTeamSources { .. }
        | Command::ObserveCheckRun { .. } | Command::ObserveLinkedPrChecks { .. }
        | Command::QueueDemand { .. } | Command::QueueSelectedWorkerDemand { .. }
        | Command::AcceptTicket { .. } | Command::RejectTicket { .. }
        | Command::StopTicket { .. } | Command::StopTicketAtRevision { .. }
                | Command::QuarantineDetachedTicket { .. }
        | Command::QuarantineExpiredTicket { .. }
        | Command::QuarantinePendingEffect { .. }
        | Command::QuarantineOrphanRunning { .. }
        | Command::QuarantineTerminalOrphan { .. }
        | Command::DisableOrchestration { .. }
                | Command::EnableOrchestration { .. }
        | Command::RenameOrchestration { .. }
        | Command::DuplicateOrchestration { .. }
        | Command::SetTeamStrategy { .. }
        | Command::SetTeamRoster { .. }
        | Command::SetTeamAttemptLimit { .. } => {
            eprintln!("SuperAgent inspection must use the read-only startup path.");
            std::process::exit(2)
        }
        Command::SyncBots { path } => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match crate::extended_bot_git::run_headless(&app, &path).await {
                    Ok(result) => {
                        for source in &result.sources {
                            println!("Source {} pinned at {} ({} manifest files).", source.id, source.commit, source.paths.len());
                        }
                        if let Some(backup) = &result.backup_path { println!("Bot configuration backup: {backup}"); }
                        for line in &result.lines { println!("{line}"); }
                        std::process::exit(if result.failed == 0 { 0 } else { 1 });
                    }
                    Err(error) => {
                        eprintln!("Git bot sync failed: {error}");
                        std::process::exit(1);
                    }
                }
            });
        }
        Command::ExportBots { path } => match crate::headless_bots::run_export(app, &path) {
            Ok(()) => {
                println!("Bot configuration written to {}.", path.display());
                std::process::exit(0)
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1)
            }
        },
        Command::SignOut => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                crate::remote::headless_sign_out(&app).await;
                println!("Signed out of PandaCode Connect.");
                std::process::exit(0)
            });
        }
        Command::FullAccess { enabled } => match crate::remote::headless_set_full_access(dir, enabled) {
            Ok(()) => {
                println!("Full access for remote tasks is {}.", if enabled { "on" } else { "off" });
                std::process::exit(0)
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1)
            }
        },
        Command::SignIn { name } => {
            let (app, dir) = (app.clone(), dir.to_owned());
            tauri::async_runtime::spawn(async move {
                if let Err(error) = crate::remote::headless_sign_in(&dir, name).await {
                    eprintln!("Sign-in failed: {error}");
                    std::process::exit(1)
                }
                crate::remote::headless_reload(&app, &dir);
                if let Some(path) = import {
                    crate::headless_bots::run_import(&app, &path);
                }
                start_services(app);
            });
        }
        Command::Serve => {
            if !crate::remote::configured() {
                eprintln!("Remote access is not configured in this build (PANDACODE_RELAY_URL / PANDACODE_GITHUB_CLIENT_ID).");
                std::process::exit(1)
            }
            if !crate::remote::signed_in(dir) {
                eprintln!("This server is not signed in to PandaCode Connect. Run: pandacode serve --sign-in");
                std::process::exit(1)
            }
            if let Some(path) = import {
                crate::headless_bots::run_import(app, &path);
            }
            // serve exists to host: flip the host switch if a saved sign-in left it off.
            if !crate::remote::host_enabled(dir) || !crate::remote::connection_enabled(dir) {
                if let Err(error) = crate::remote::headless_set_host_enabled(dir, true) {
                    eprintln!("Could not enable hosting: {error}");
                    std::process::exit(1)
                }
                crate::remote::headless_reload(app, dir);
                println!("Hosting and PandaCode Connect are enabled for this headless server.");
            }
            start_services(app.clone());
        }
    }
}

/// The serve-only services: Ctrl-C/SIGTERM, one status line for journald, native limit
/// resumes, and the queue the previous run left behind.
fn start_services(app: tauri::AppHandle) {
    let signals = app.clone();
    tauri::async_runtime::spawn(async move {
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = term.recv() => {},
            }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
        println!("Shutting down; waiting for running agents (up to 12s)…");
        signals.exit(0);
    });
    let status = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last = String::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let line = crate::remote::headless_status(&status);
            if line != last {
                println!("{line}");
                last = line;
            }
        }
    });
    let watchdog = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let state = watchdog.state::<AppState>();
            if state.shutdown.load(Ordering::SeqCst) != 0 {
                break;
            }
            // Lock ordering matches admission: running agents first, then Store.
            // A separate one-shot CLI can revoke a Ticket in the same SQLite
            // database but cannot reach this process's provider handle directly.
            // Stop those still-running native sessions on the next bounded tick.
            let running_sessions = match state.agents.lock() {
                Ok(agents) => agents.keys().cloned().collect::<Vec<_>>(),
                Err(error) => {
                    eprintln!("serve: SuperAgent running session inspection failed: {error}");
                    continue;
                }
            };
            let cancellations = match state.store.lock() {
                Ok(store) => crate::superagent::watchdog_expired_assignments(
                    &store, crate::storage::now(),
                ).and_then(|expired| {
                    let revoked = crate::superagent::headless_cancelled_assignments(
                        &store, &running_sessions,
                    )?;
                    Ok(expired.into_iter().chain(revoked).collect::<BTreeSet<_>>())
                }),
                Err(error) => Err(error.to_string()),
            };
            match cancellations {
                Ok(sessions) => for session_id in sessions {
                    println!("serve: stopping revoked or expired SuperAgent assignment {session_id}");
                    if let Err(error) = crate::stop_session(
                        watchdog.clone(), watchdog.state::<AppState>(), session_id,
                    ).await {
                        eprintln!("serve: SuperAgent session stop failed: {error}");
                    }
                },
                Err(error) => eprintln!("serve: SuperAgent watchdog failed: {error}"),
            }
        }
    });
    let limits = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let state = limits.state::<AppState>();
            if state.shutdown.load(Ordering::SeqCst) != 0 {
                break;
            }
            let due = state
                .store
                .lock()
                .ok()
                .and_then(|store| store.due_limit_resumes(crate::storage::now() + 1000).ok())
                .unwrap_or_default();
            for session_id in due {
                if state.agents.lock().map(|agents| agents.contains_key(&session_id)).unwrap_or(true) {
                    continue;
                }
                // Generic limit recovery must not start another turn on an
                // old SuperAgent assignment. Its mandate and the one-shot
                // provider attempt cannot be silently renewed.
                let ordinary_session = match state.store.lock() {
                    Ok(store) => crate::superagent::has_task_assignment_history(&store, &session_id),
                    Err(error) => Err(error.to_string()),
                };
                match ordinary_session {
                    Ok(false) => {}
                    Ok(true) => continue,
                    Err(error) => {
                        eprintln!("serve: skipping usage-limit resume after native session lookup failed: {error}");
                        continue;
                    }
                }
                println!("serve: resuming {session_id} at its usage-limit reset");
                if let Err(error) = crate::send_message(
                    limits.clone(),
                    limits.state::<AppState>(),
                    session_id,
                    "Continue where you left off.".into(),
                    None,
                    None,
                )
                .await
                {
                    eprintln!("serve: the limit resume could not be sent: {error}");
                }
            }
        }
    });
    // Queued messages from before the restart: remote clients have no Resume button, so
    // serve delivers them once the resumed cloud runs have had a moment to re-attach.
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let state = app.state::<AppState>();
        let queued: Vec<String> = state
            .store
            .lock()
            .ok()
            .and_then(|store| message_queue::list(&store, None).ok())
            .map(|entries| entries.into_iter().map(|entry| entry.session_id).collect())
            .unwrap_or_default();
        for session_id in queued.into_iter().collect::<BTreeSet<_>>() {
            drain_queue(app.clone(), session_id);
        }
    });
}

/// Sends the next queued message of a finished task — the window's dispatchQueue (main.ts)
/// and the bot gateway's native drain, for tasks that have neither. Holds the queue like
/// the desktop does: archived or still-limited tasks wait (the limit-resume send ends a
/// run, which then drains), and bot conversations are the gateway's. Sync: spawns the
/// send itself, so callers can chain it after their own run-end work.
pub(crate) fn drain_queue(app: tauri::AppHandle, session_id: String) {
    tauri::async_runtime::spawn(async move {
        let now = crate::storage::now();
        let eligible = {
            let state = app.state::<AppState>();
            if state.agents.lock().map(|agents| agents.contains_key(&session_id)).unwrap_or(true) {
                return;
            }
            let Ok(store) = state.store.lock() else { return };
            // A settled or interrupted SuperAgent assignment remains a task
            // forever. Never reinterpret its old session as a generic queued
            // conversation after the native bot-thread link disappears.
            if crate::superagent::has_task_assignment_history(&store, &session_id)
                .unwrap_or(true)
            {
                return;
            }
            // Bot conversations drain through the bot gateway.
            if crate::bots::owner_of(&store, &session_id).ok().flatten().is_some() {
                return;
            }
            store.session(&session_id).ok().is_some_and(|task| can_drain(task.archived, task.limited_at, task.limited_until, now))
        };
        if !eligible {
            return;
        }
        let next = {
            let state = app.state::<AppState>();
            let Ok(store) = state.store.lock() else { return };
            message_queue::list(&store, Some(&session_id)).ok().and_then(|entries| entries.into_iter().next())
        };
        let Some(entry) = next else { return };
        let entry = {
            let state = app.state::<AppState>();
            let Ok(store) = state.store.lock() else { return };
            match message_queue::take(&store, &entry.id) {
                Ok(entry) => entry,
                Err(error) => {
                    eprintln!("serve: could not take the next queued message: {error}");
                    return;
                }
            }
        };
        let (context, image_ids) = crate::bots::gateway::queued_prompt(&entry.attachments);
        let images = (!image_ids.is_empty()).then_some(image_ids);
        println!("serve: sending the next queued message to {session_id}");
        if let Err(error) = crate::send_message(
            app.clone(),
            app.state::<AppState>(),
            session_id.clone(),
            format!("{}{context}", entry.text),
            images,
            None,
        )
        .await
        {
            // A failed launch puts the entry back first; no run ends, so the queue holds.
            let state = app.state::<AppState>();
            if let Ok(store) = state.store.lock() {
                if let Err(lost) = message_queue::restore(&store, &entry) {
                    eprintln!("serve: queued message lost after a failed launch: {lost}");
                }
            }
            eprintln!("serve: couldn't send the next queued message: {error}");
        }
    });
}

/// A task may take its next queued message when it is live and not inside a usage limit —
/// limitActive in src/usage-limit.ts: a limit without a known reset holds indefinitely.
fn can_drain(archived: bool, limited_at: Option<i64>, limited_until: Option<i64>, now: i64) -> bool {
    !archived && limited_at.is_none_or(|_at| limited_until.is_some_and(|until| until <= now))
}

/// An agent tool or a remote client queued (or removed) a message. The run-end hook drains
/// finished tasks; this covers a message queued while the task sits idle.
pub(crate) fn queue_changed(app: &tauri::AppHandle, session_id: &str) {
    if !active() {
        return;
    }
    drain_queue(app.clone(), session_id.to_owned());
}

fn parse(args: &[String]) -> Result<(Command, Option<PathBuf>), String> {
    let mut name: Option<String> = None;
    let mut import: Option<PathBuf> = None;
    let mut sign_in = false;
    let mut command = Command::Serve;
    let mut ops = 0;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => return Err(help()),
            "--sign-in" => {
                sign_in = true;
                ops += 1;
            }
            "--sign-out" => {
                command = Command::SignOut;
                ops += 1;
            }
            "--full-access" => {
                command = Command::FullAccess { enabled: true };
                ops += 1;
            }
            "--no-full-access" => {
                command = Command::FullAccess { enabled: false };
                ops += 1;
            }
            "--export-bots" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("--export-bots needs a file path.\n\n{}", help()))?;
                command = Command::ExportBots { path: PathBuf::from(value) };
                ops += 1;
                index += 1;
            }
            "--preview-orchestrations" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--preview-orchestrations needs a file path.\n\n{}", help()))?;
                command = Command::PreviewOrchestrations { path: PathBuf::from(value) };
                ops += 1;
                index += 1;
            }
            "--import-orchestrations" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--import-orchestrations needs a file path.\n\n{}", help()))?;
                command = Command::ImportOrchestrations { path: PathBuf::from(value) };
                ops += 1;
                index += 1;
            }
            "--export-orchestrations" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--export-orchestrations needs a file path.\n\n{}", help()))?;
                command = Command::ExportOrchestrations { path: PathBuf::from(value) };
                ops += 1;
                index += 1;
            }
            "--inspect-orchestrations" => {
                command = Command::InspectOrchestrations;
                ops += 1;
            }
            "--create-orchestration" => {
                let file = args.get(index + 1)
                    .ok_or("--create-orchestration needs a local Team JSON file.")?;
                if file.is_empty() || file.starts_with("--") {
                    return Err("Provide a local Team creation file path, not another command.".into());
                }
                command = Command::CreateOrchestration { config_file: PathBuf::from(file) };
                ops += 1;
                index += 1;
            }
            "--list-tickets" => {
                command = Command::ListTickets;
                ops += 1;
            }
            "--list-tickets-page" => {
                let (before_created_at, before_id) =
                    if args.get(index + 1).map(String::as_str) == Some("--before") {
                        let at = args.get(index + 2).and_then(|s| s.parse::<i64>().ok())
                            .filter(|at| *at > 0)
                            .ok_or("--before needs a positive Ticket creation timestamp.")?;
                        let id = args.get(index + 3)
                            .ok_or("--before needs a Ticket creation timestamp and Ticket ID.")?;
                        validate_ticket_id(id)?;
                        index += 3;
                        (Some(at),Some(id.clone()))
                    } else { (None,None) };
                command = Command::ListTicketsPage { before_created_at, before_id };
                ops += 1;
            }
            "--list-ticket-activity" => {
                let ticket_id = args.get(index + 1)
                    .ok_or("--list-ticket-activity needs a Ticket ID.")?;
                validate_ticket_id(ticket_id)?;
                let before_id = if args.get(index + 2).map(String::as_str) == Some("--before") {
                    let value = args.get(index + 3)
                        .ok_or("--before needs a positive audit event ID.")?;
                    let parsed = value.parse::<i64>().ok().filter(|id| *id > 0)
                        .ok_or("--before needs a positive audit event ID.")?;
                    index += 2;
                    Some(parsed)
                } else { None };
                command = Command::ListTicketActivity { ticket_id: ticket_id.clone(), before_id };
                ops += 1;
                index += 1;
            }
            "--list-team-member-activity" => {
                let team_id = args.get(index + 1)
                    .ok_or("--list-team-member-activity needs a Team ID and bot ID.")?;
                validate_orchestration_id(team_id)?;
                let bot_id = args.get(index + 2)
                    .ok_or("--list-team-member-activity needs a Team ID and bot ID.")?;
                validate_member_identity(bot_id)?;
                let (before_created_at, before_assignment_id) =
                    if args.get(index + 3).map(String::as_str) == Some("--before") {
                        let at = args.get(index + 4)
                            .ok_or("--before needs a positive creation timestamp and assignment ID.")?
                            .parse::<i64>().ok().filter(|value| *value > 0)
                            .ok_or("--before needs a positive creation timestamp and assignment ID.")?;
                        let assignment_id = args.get(index + 5)
                            .ok_or("--before needs a positive creation timestamp and assignment ID.")?;
                        validate_member_identity(assignment_id)?;
                        index += 3;
                        (Some(at), Some(assignment_id.clone()))
                    } else { (None, None) };
                command = Command::ListTeamMemberActivity {
                    orchestration_id: team_id.clone(), bot_id: bot_id.clone(),
                    before_created_at, before_assignment_id,
                };
                ops += 1;
                index += 2;
            }
            "--list-recovery-inbox" => {
                let (before_priority,before_updated_at,before_id)=
                    if args.get(index+1).map(String::as_str)==Some("--before") {
                        let priority=args.get(index+2).ok_or(
                            "--before requires priority 0..2, timestamp and Ticket ID."
                        )?.parse::<i64>().map_err(|_|"Invalid recovery cursor priority.")?;
                        if !(0..=2).contains(&priority) {
                            return Err("Recovery priority must be between 0 and 2.".into());
                        }
                        let updated_at=args.get(index+3).ok_or(
                            "--before requires priority 0..2, timestamp and Ticket ID."
                        )?.parse::<i64>().map_err(|_|"Invalid recovery cursor timestamp.")?;
                        if updated_at<=0 {return Err("Recovery cursor timestamp must be positive.".into());}
                        let id=args.get(index+4).ok_or(
                            "--before requires priority 0..2, timestamp and Ticket ID."
                        )?;
                        validate_ticket_id(id)?;
                        index+=4;
                        (Some(priority),Some(updated_at),Some(id.clone()))
                    } else {(None,None,None)};
                command = Command::ListRecoveryInbox { before_priority,before_updated_at,before_id };
                ops += 1;
            }
            "--list-team-recovery-inbox" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--list-team-recovery-inbox needs a Team ID.")?;
                validate_orchestration_id(orchestration_id)?;
                let (before_priority, before_updated_at, before_id) =
                    if args.get(index + 2).map(String::as_str) == Some("--before") {
                        let priority = args.get(index + 3)
                            .ok_or("--before needs recovery priority, timestamp and Ticket ID.")?
                            .parse::<i64>().ok().filter(|p| (0..=2).contains(p))
                            .ok_or("Invalid Team recovery priority (0..2).")?;
                        let updated_at = args.get(index + 4)
                            .ok_or("--before needs recovery priority, timestamp and Ticket ID.")?
                            .parse::<i64>().ok().filter(|at| *at > 0)
                            .ok_or("Invalid Team recovery timestamp.")?;
                        let ticket_id = args.get(index + 5)
                            .ok_or("--before needs recovery priority, timestamp and Ticket ID.")?;
                        validate_ticket_id(ticket_id)?;
                        index += 4;
                        (Some(priority), Some(updated_at), Some(ticket_id.clone()))
                    } else { (None, None, None) };
                command = Command::ListTeamRecoveryInbox {
                    orchestration_id: orchestration_id.clone(),
                    before_priority, before_updated_at, before_id,
                };
                ops += 1;
                index += 1;
            }
            "--list-unresolved-effects" => {
                let (before_updated_at,before_ticket_id,before_effect_key) =
                    if args.get(index+1).map(String::as_str)==Some("--before") {
                        let at=args.get(index+2)
                            .ok_or("--before needs timestamp, Ticket ID and effect key.")?
                            .parse::<i64>().ok().filter(|at| *at>0)
                            .ok_or("Unresolved effect timestamp must be positive.")?;
                        let ticket=args.get(index+3)
                            .ok_or("--before needs a Ticket ID.")?;
                        validate_ticket_id(ticket)?;
                        let effect=args.get(index+4)
                            .ok_or("--before needs an effect key.")?;
                        validate_effect_key(effect)?;
                        index+=4;
                        (Some(at),Some(ticket.clone()),Some(effect.clone()))
                    } else {(None,None,None)};
                command=Command::ListUnresolvedEffects {
                    before_updated_at,before_ticket_id,before_effect_key,
                };
                ops+=1;
            }
            "--list-team-unresolved-effects" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--list-team-unresolved-effects needs a Team ID.")?;
                validate_orchestration_id(orchestration_id)?;
                let (before_updated_at,before_ticket_id,before_effect_key) =
                    if args.get(index+2).map(String::as_str)==Some("--before") {
                        let at=args.get(index+3)
                            .ok_or("--before needs timestamp, Ticket ID and effect key.")?
                            .parse::<i64>().ok().filter(|at| *at>0)
                            .ok_or("Unresolved effect timestamp must be positive.")?;
                        let ticket=args.get(index+4)
                            .ok_or("--before needs a Ticket ID.")?;
                        validate_ticket_id(ticket)?;
                        let effect=args.get(index+5)
                            .ok_or("--before needs an effect key.")?;
                        validate_effect_key(effect)?;
                        index+=4;
                        (Some(at),Some(ticket.clone()),Some(effect.clone()))
                    } else {(None,None,None)};
                command=Command::ListTeamUnresolvedEffects {
                    orchestration_id:orchestration_id.clone(),
                    before_updated_at,before_ticket_id,before_effect_key,
                };
                ops+=1;
                index+=1;
            }
            "--list-review-inbox" => {
                let (before_updated_at, before_id) =
                    if args.get(index + 1).map(String::as_str) == Some("--before") {
                        let updated_at = args.get(index + 2)
                            .ok_or("--before needs a positive millisecond timestamp and Ticket ID.")?
                            .parse::<i64>().ok().filter(|value| *value > 0)
                            .ok_or("Review cursor timestamp must be positive.")?;
                        let id = args.get(index + 3)
                            .ok_or("--before needs the previous page's last Ticket ID.")?;
                        validate_ticket_id(id)?;
                        index += 3;
                        (Some(updated_at), Some(id.clone()))
                    } else { (None, None) };
                command = Command::ListReviewInbox { before_updated_at, before_id };
                ops += 1;
            }
            "--list-team-review-inbox" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--list-team-review-inbox needs a Team ID.")?;
                validate_orchestration_id(orchestration_id)?;
                let (before_updated_at, before_id) =
                    if args.get(index + 2).map(String::as_str) == Some("--before") {
                        let updated_at = args.get(index + 3)
                            .ok_or("--before needs a positive millisecond timestamp and Ticket ID.")?
                            .parse::<i64>().ok().filter(|value| *value > 0)
                            .ok_or("Team review cursor timestamp must be positive.")?;
                        let id = args.get(index + 4)
                            .ok_or("--before needs the previous page's last Ticket ID.")?;
                        validate_ticket_id(id)?;
                        index += 3;
                        (Some(updated_at), Some(id.clone()))
                    } else { (None, None) };
                command = Command::ListTeamReviewInbox {
                    orchestration_id: orchestration_id.clone(), before_updated_at, before_id,
                };
                ops += 1;
                index += 1;
            }
            "--list-demands" => {
                let state = args.get(index + 1)
                    .ok_or_else(|| format!("--list-demands needs a native Ticket state or all.\n\n{}", help()))?;
                if !matches!(state.as_str(), "all" | "queued" | "running" |
                    "waiting_approval" | "waiting_review" | "blocked" | "completed" |
                    "failed" | "stopped" | "interrupted" | "inconclusive") {
                    return Err("Choose an exact native Ticket state or all.".into());
                }
                let (before_updated_at, before_id) =
                    if args.get(index + 2).map(String::as_str) == Some("--before") {
                        let timestamp = args.get(index + 3)
                            .ok_or("--before needs a positive millisecond timestamp and Ticket ID.")?
                            .parse::<i64>().ok().filter(|value| *value > 0)
                            .ok_or("--before needs a positive millisecond timestamp.")?;
                        let cursor_id = args.get(index + 4)
                            .ok_or("--before needs the previous page's last Ticket ID.")?;
                        validate_ticket_id(cursor_id)?;
                        index += 3;
                        (Some(timestamp), Some(cursor_id.clone()))
                    } else { (None, None) };
                let mut search_query = None;
                let mut project_id = None;
                loop {
                    match args.get(index + 2).map(String::as_str) {
                        Some("--search") if search_query.is_none() => {
                            let query = args.get(index + 3)
                                .ok_or("--search needs a 1 to 128 byte objective fragment.")?;
                            if query.trim().is_empty() || query.len() > 128
                                || query.starts_with("--") || query.chars().any(char::is_control) {
                                return Err("--search needs 1 to 128 non-control UTF-8 bytes.".into());
                            }
                            search_query = Some(query.clone());
                            index += 2;
                        }
                        Some("--project") if project_id.is_none() => {
                            let value = args.get(index + 3)
                                .ok_or("--project needs an exact admitted native Project ID.")?;
                            validate_project_id(value)?;
                            project_id = Some(value.clone());
                            index += 2;
                        }
                        _ => break,
                    }
                }
                command = Command::ListDemands {
                    state: state.clone(), before_updated_at, before_id,
                    search_query, project_id,
                };
                ops += 1;
                index += 1;
            }
            "--list-team-demands" => {
                let id = args.get(index + 1)
                    .ok_or_else(|| format!("--list-team-demands needs a Team ID and state.\n\n{}", help()))?;
                validate_orchestration_id(id)?;
                let state = args.get(index + 2)
                    .ok_or_else(|| format!("--list-team-demands needs a state.\n\n{}", help()))?;
                if !matches!(state.as_str(), "all" | "queued" | "running" |
                    "waiting_approval" | "waiting_review" | "blocked" | "completed" |
                    "failed" | "stopped" | "interrupted" | "inconclusive") {
                    return Err("Choose an exact native Ticket state or all.".into());
                }
                let (before_updated_at, before_id) =
                    if args.get(index + 3).map(String::as_str) == Some("--before") {
                        let timestamp = args.get(index + 4)
                            .ok_or("--before needs a positive millisecond timestamp and Ticket ID.")?
                            .parse::<i64>().ok().filter(|value| *value > 0)
                            .ok_or("--before needs a positive millisecond timestamp.")?;
                        let cursor_id = args.get(index + 5)
                            .ok_or("--before needs the previous page's last Ticket ID.")?;
                        validate_ticket_id(cursor_id)?;
                        index += 3;
                        (Some(timestamp), Some(cursor_id.clone()))
                    } else { (None, None) };
                let mut search_query = None;
                let mut project_id = None;
                loop {
                    match args.get(index + 3).map(String::as_str) {
                        Some("--search") if search_query.is_none() => {
                            let query = args.get(index + 4)
                                .ok_or("--search needs a 1 to 128 byte objective fragment.")?;
                            if query.trim().is_empty() || query.len() > 128
                                || query.starts_with("--") || query.chars().any(char::is_control) {
                                return Err("--search needs 1 to 128 non-control UTF-8 bytes.".into());
                            }
                            search_query = Some(query.clone());
                            index += 2;
                        }
                        Some("--project") if project_id.is_none() => {
                            let value = args.get(index + 4)
                                .ok_or("--project needs an exact admitted native Project ID.")?;
                            validate_project_id(value)?;
                            project_id = Some(value.clone());
                            index += 2;
                        }
                        _ => break,
                    }
                }
                command = Command::ListTeamDemands {
                    orchestration_id: id.clone(), state: state.clone(),
                    before_updated_at, before_id, search_query, project_id,
                };
                ops += 1;
                index += 2;
            }
            "--inspect-team-roster" => {
                let value=args.get(index+1)
                    .ok_or_else(||format!("--inspect-team-roster needs a Team ID.\n\n{}",help()))?;
                validate_orchestration_id(value)?;
                command=Command::InspectTeamRoster{orchestration_id:value.clone()};
                ops+=1;
                index+=1;
            }
            "--preflight-demand" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--preflight-demand needs a Team ID, mode and objective file.")?;
                validate_orchestration_id(orchestration_id)?;
                let mode = args.get(index + 2)
                    .ok_or("--preflight-demand needs direct or coordinated mode.")?;
                let coordinated = match mode.as_str() {
                    "direct" => false,
                    "coordinated" => true,
                    _ => return Err("Choose direct or coordinated native Demand preflight mode.".into()),
                };
                let path = args.get(index + 3)
                    .ok_or("--preflight-demand needs a local UTF-8 objective file.")?;
                if path.is_empty() || path.starts_with("--") {
                    return Err("Provide an objective file path, not another command.".into());
                }
                command = Command::PreflightDemand {
                    orchestration_id: orchestration_id.clone(), coordinated,
                    objective_file: PathBuf::from(path),
                };
                ops += 1;
                index += 3;
            }
            "--preflight-worker-demand" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--preflight-worker-demand needs Team ID, Worker ID and objective file.")?;
                validate_orchestration_id(orchestration_id)?;
                let worker_bot_id = args.get(index + 2)
                    .ok_or("--preflight-worker-demand needs a configured Worker bot ID.")?;
                validate_member_identity(worker_bot_id)?;
                let path = args.get(index + 3)
                    .ok_or("--preflight-worker-demand needs a local UTF-8 objective file.")?;
                if path.is_empty() || path.starts_with("--") {
                    return Err("Provide an objective file path, not another command.".into());
                }
                command = Command::PreflightWorkerDemand {
                    orchestration_id: orchestration_id.clone(),
                    worker_bot_id: worker_bot_id.clone(),
                    objective_file: PathBuf::from(path),
                };
                ops += 1;
                index += 3;
            }
            "--inspect-team-readiness" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--inspect-team-readiness needs a Team ID.\n\n{}", help()))?;
                validate_orchestration_id(value)?;
                command = Command::InspectTeamReadiness { orchestration_id: value.clone() };
                ops += 1;
                index += 1;
            }
            "--inspect-source-health" => {
                let id = args.get(index + 1)
                    .ok_or("--inspect-source-health needs a source ID.")?;
                validate_ticket_id(id)
                    .map_err(|_| "Provide a bounded source ID.".to_owned())?;
                command = Command::InspectSourceHealth { source_id: id.clone() };
                ops += 1;
                index += 1;
            }
            "--inspect-source-decisions" => {
                let id = args.get(index + 1)
                    .ok_or("--inspect-source-decisions needs a source ID.")?;
                validate_ticket_id(id)
                    .map_err(|_| "Provide a bounded source ID.".to_owned())?;
                let mut paginated = false;
                let mut before_sequence = None;
                let mut before_delivery_id = None;
                match args.get(index + 2).map(String::as_str) {
                    Some("--page") => {
                        paginated = true;
                        index += 1;
                    }
                    Some("--before") => {
                        let sequence = args.get(index + 3)
                            .ok_or("--before requires a positive source sequence and delivery ID.")?
                            .parse::<i64>().ok().filter(|value| *value > 0)
                            .ok_or("--before requires a positive source sequence.")?;
                        let delivery_id = args.get(index + 4)
                            .ok_or("--before requires the previous page's delivery ID.")?;
                        validate_ticket_id(delivery_id)?;
                        before_sequence = Some(sequence);
                        before_delivery_id = Some(delivery_id.clone());
                        paginated = true;
                        index += 3;
                    }
                    _ => {}
                }
                command = Command::InspectSourceDecisions {
                    source_id: id.clone(), paginated, before_sequence, before_delivery_id,
                };
                ops += 1;
                index += 1;
            }
            "--create-source" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--create-source needs Team ID, owner/repo, actor and event kind.")?;
                validate_orchestration_id(orchestration_id)?;
                let repository = args.get(index + 2)
                    .ok_or("--create-source needs an exact owner/repo.")?;
                let actor_filter = args.get(index + 3)
                    .ok_or("--create-source needs an exact GitHub actor.")?;
                let event_kind = args.get(index + 4)
                    .ok_or("--create-source needs a supported event kind.")?;
                if [repository, actor_filter, event_kind].iter()
                    .any(|value| value.is_empty() || value.starts_with("--"))
                {
                    return Err("Source fields must be explicit values, not CLI switches.".into());
                }
                command = Command::CreateSource {
                    orchestration_id: orchestration_id.clone(),
                    repository: repository.clone(),
                    actor_filter: actor_filter.clone(),
                    event_kind: event_kind.clone(),
                };
                ops += 1;
                index += 4;
            }
            "--list-team-sources" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--list-team-sources needs a Team ID.")?;
                validate_orchestration_id(orchestration_id)?;
                command = Command::ListTeamSources {
                    orchestration_id: orchestration_id.clone(),
                };
                ops += 1;
                index += 1;
            }
            "--set-source-mode" => {
                let source_id = args.get(index + 1)
                    .ok_or("--set-source-mode needs a source ID, expected revision and off or observe.")?;
                validate_ticket_id(source_id)
                    .map_err(|_| "Provide a bounded alphanumeric source ID.".to_owned())?;
                let revision = args.get(index + 2)
                    .ok_or("--set-source-mode needs an expected source policy revision.")?
                    .parse::<i64>().ok().filter(|value| *value > 0)
                    .ok_or("Provide a positive expected source policy revision.")?;
                let mode = args.get(index + 3)
                    .ok_or("--set-source-mode needs off or observe.")?;
                if mode != "off" && mode != "observe" {
                    return Err("Only off or observe is supported; Auto has no native grant.".into());
                }
                command = Command::SetSourceMode {
                    source_id: source_id.clone(), expected_revision: revision, mode: mode.clone(),
                };
                ops += 1;
                index += 3;
            }
            "--observe-source" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--observe-source needs a source ID.\n\n{}", help()))?;
                validate_ticket_id(value).map_err(|_| "Provide a bounded alphanumeric source ID.".to_owned())?;
                command = Command::ObserveSource { source_id: value.clone() };
                ops += 1;
                index += 1;
            }
            "--observe-team-sources" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--observe-team-sources needs a Team ID.\\n\\n{}", help()))?;
                validate_orchestration_id(value)?;
                command = Command::ObserveTeamSources { orchestration_id: value.clone() };
                ops += 1;
                index += 1;
            }
            "--observe-check-run" => {
                let source_id = args.get(index + 1)
                    .ok_or_else(|| format!("--observe-check-run needs a source ID and Check Run ID.\n\n{}", help()))?;
                validate_ticket_id(source_id)
                    .map_err(|_| "Provide a bounded alphanumeric source ID.".to_owned())?;
                let raw_id = args.get(index + 2)
                    .ok_or("--observe-check-run needs a positive Check Run ID.")?;
                let check_run_id = raw_id.parse::<i64>().ok().filter(|id| *id > 0)
                    .ok_or("--observe-check-run needs a positive Check Run ID.")?;
                command = Command::ObserveCheckRun { source_id: source_id.clone(), check_run_id };
                ops += 1;
                index += 2;
            }
            "--observe-linked-pr-checks" => {
                let source_id = args.get(index + 1)
                    .ok_or("--observe-linked-pr-checks needs source, Ticket and pull:123.")?;
                validate_member_identity(source_id)?;
                let ticket_id = args.get(index + 2)
                    .ok_or("--observe-linked-pr-checks needs a Ticket ID.")?;
                validate_ticket_id(ticket_id)?;
                let pull_key = args.get(index + 3)
                    .ok_or("--observe-linked-pr-checks needs a canonical pull:123 key.")?;
                if !crate::superagent::events::valid_thread_key(pull_key)
                    || !pull_key.starts_with("pull:") {
                    return Err("--observe-linked-pr-checks requires an exact pull:123 key.".into());
                }
                command = Command::ObserveLinkedPrChecks {
                    source_id: source_id.clone(), ticket_id: ticket_id.clone(),
                    pull_key: pull_key.clone(),
                };
                ops += 1;
                index += 3;
            }
            "--queue-worker-demand" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--queue-worker-demand needs a Team ID, Worker ID, request key and objective file.")?;
                validate_orchestration_id(orchestration_id)?;
                let worker_bot_id = args.get(index + 2)
                    .ok_or("--queue-worker-demand needs a configured Worker bot ID.")?;
                validate_ticket_id(worker_bot_id)
                    .map_err(|_| "Provide a bounded exact Worker bot ID.".to_owned())?;
                let request_key = args.get(index + 3)
                    .ok_or("--queue-worker-demand needs a stable request key.")?;
                if request_key.is_empty() || request_key.len() > 128
                    || !request_key.bytes().all(|c| c.is_ascii_alphanumeric()
                        || matches!(c, b'-' | b'_' | b'.')) {
                    return Err("Provide a bounded alphanumeric request key.".into());
                }
                let path = args.get(index + 4)
                    .ok_or("--queue-worker-demand needs a local UTF-8 objective file.")?;
                if path.is_empty() || path.starts_with("--") {
                    return Err("Provide an objective file path, not another command.".into());
                }
                command = Command::QueueSelectedWorkerDemand {
                    orchestration_id: orchestration_id.clone(),
                    worker_bot_id: worker_bot_id.clone(),
                    request_key: request_key.clone(), objective_file: PathBuf::from(path),
                };
                ops += 1;
                index += 4;
            }
            "--queue-demand" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--queue-demand needs a Team ID, request key and objective file.")?;
                validate_orchestration_id(orchestration_id)?;
                let request_key = args.get(index + 2)
                    .ok_or("--queue-demand needs a stable request key.")?;
                if request_key.is_empty() || request_key.len() > 128
                    || !request_key.bytes().all(|c| c.is_ascii_alphanumeric()
                        || matches!(c, b'-' | b'_' | b'.')) {
                    return Err("Provide a bounded alphanumeric request key.".into());
                }
                let path = args.get(index + 3)
                    .ok_or("--queue-demand needs a local UTF-8 objective file.")?;
                if path.is_empty() || path.starts_with("--") {
                    return Err("Provide an objective file path, not another command.".into());
                }
                command = Command::QueueDemand {
                    orchestration_id: orchestration_id.clone(),
                    request_key: request_key.clone(), objective_file: PathBuf::from(path),
                };
                ops += 1;
                index += 3;
            }
            "--inspect-ticket-source" => {
                let ticket_id = args.get(index + 1)
                    .ok_or_else(|| format!("--inspect-ticket-source needs a Ticket ID.\n\n{}", help()))?;
                validate_ticket_id(ticket_id)?;
                let mut paginated = false;
                let mut before_at = None;
                let mut before_source_id = None;
                let mut before_delivery_id = None;
                match args.get(index + 2).map(String::as_str) {
                    Some("--page") => {
                        paginated = true;
                        index += 1;
                    }
                    Some("--before") => {
                        let at = args.get(index + 3)
                            .ok_or("--before needs timestamp, source ID and delivery ID.")?
                            .parse::<i64>().ok().filter(|at| *at > 0)
                            .ok_or("--before needs a positive native receipt timestamp.")?;
                        let source = args.get(index + 4)
                            .ok_or("--before needs a source ID.")?;
                        let delivery = args.get(index + 5)
                            .ok_or("--before needs a delivery ID.")?;
                        validate_ticket_id(source)?;
                        validate_ticket_id(delivery)?;
                        paginated = true;
                        before_at = Some(at);
                        before_source_id = Some(source.clone());
                        before_delivery_id = Some(delivery.clone());
                        // The cursor has four arguments after the Ticket ID:
                        // --before, timestamp, source ID and delivery ID.
                        // The common case and loop each advance once more.
                        index += 4;
                    }
                    _ => {}
                }
                command = Command::InspectTicketSource {
                    ticket_id:ticket_id.clone(), paginated,
                    before_at,before_source_id,before_delivery_id,
                };
                ops += 1;
                index += 1;
            }
            "--list-ticket-source-links" => {
                let ticket_id = args.get(index + 1)
                    .ok_or("--list-ticket-source-links needs a Ticket ID.")?;
                validate_ticket_id(ticket_id)?;
                command = Command::ListTicketSourceLinks { ticket_id: ticket_id.clone() };
                ops += 1;
                index += 1;
            }
            "--link-source-thread" => {
                let source_id = args.get(index + 1)
                    .ok_or("--link-source-thread needs source ID, Ticket ID and exact thread key.")?;
                let ticket_id = args.get(index + 2)
                    .ok_or("--link-source-thread needs a Ticket ID.")?;
                let subject_key = args.get(index + 3)
                    .ok_or("--link-source-thread needs an exact issue:123 or pull:123 key.")?;
                validate_ticket_id(source_id).map_err(|_| "Provide a bounded source ID.".to_owned())?;
                validate_ticket_id(ticket_id)?;
                if !crate::superagent::events::valid_thread_key(subject_key) {
                    return Err("Provide an exact issue:123 or pull:123 thread key.".into());
                }
                command = Command::LinkSourceThread {
                    source_id:source_id.clone(),ticket_id:ticket_id.clone(),
                    subject_key:subject_key.clone(),
                };
                ops += 1;
                index += 3;
            }
            "--link-issue-pull" => {
                let source_id = args.get(index + 1)
                    .ok_or("--link-issue-pull needs source ID, Ticket ID, issue key and PR key.")?;
                let ticket_id = args.get(index + 2)
                    .ok_or("--link-issue-pull needs a Ticket ID.")?;
                let issue_key = args.get(index + 3)
                    .ok_or("--link-issue-pull needs an exact issue:123 key.")?;
                let pull_key = args.get(index + 4)
                    .ok_or("--link-issue-pull needs an exact pull:123 key.")?;
                validate_ticket_id(source_id).map_err(|_| "Provide a bounded source ID.".to_owned())?;
                validate_ticket_id(ticket_id)?;
                if !issue_key.starts_with("issue:")
                    || !crate::superagent::events::valid_thread_key(issue_key)
                    || !pull_key.starts_with("pull:")
                    || !crate::superagent::events::valid_thread_key(pull_key)
                {
                    return Err("Provide exact issue:123 and pull:123 context keys.".into());
                }
                command = Command::LinkIssuePull {
                    source_id:source_id.clone(), ticket_id:ticket_id.clone(),
                    issue_key:issue_key.clone(), pull_key:pull_key.clone(),
                };
                ops += 1;
                index += 4;
            }
            "--verify-link-issue-pull" => {
                let source_id = args.get(index + 1)
                    .ok_or("--verify-link-issue-pull needs source ID, Ticket ID, issue key and PR key.")?;
                let ticket_id = args.get(index + 2)
                    .ok_or("--verify-link-issue-pull needs a Ticket ID.")?;
                let issue_key = args.get(index + 3)
                    .ok_or("--verify-link-issue-pull needs an exact issue:123 key.")?;
                let pull_key = args.get(index + 4)
                    .ok_or("--verify-link-issue-pull needs an exact pull:123 key.")?;
                validate_ticket_id(source_id).map_err(|_| "Provide a bounded source ID.".to_owned())?;
                validate_ticket_id(ticket_id)?;
                if !issue_key.starts_with("issue:")
                    || !crate::superagent::events::valid_thread_key(issue_key)
                    || !pull_key.starts_with("pull:")
                    || !crate::superagent::events::valid_thread_key(pull_key)
                {
                    return Err("Provide exact issue:123 and pull:123 context keys.".into());
                }
                command = Command::VerifyLinkIssuePull {
                    source_id:source_id.clone(), ticket_id:ticket_id.clone(),
                    issue_key:issue_key.clone(), pull_key:pull_key.clone(),
                };
                ops += 1;
                index += 4;
            }
            "--unlink-source-thread" => {
                let source_id = args.get(index + 1)
                    .ok_or("--unlink-source-thread needs source ID, Ticket ID and exact thread key.")?;
                let ticket_id = args.get(index + 2)
                    .ok_or("--unlink-source-thread needs a Ticket ID.")?;
                let subject_key = args.get(index + 3)
                    .ok_or("--unlink-source-thread needs an exact issue:123 or pull:123 key.")?;
                validate_ticket_id(source_id).map_err(|_| "Provide a bounded source ID.".to_owned())?;
                validate_ticket_id(ticket_id)?;
                if !crate::superagent::events::valid_thread_key(subject_key) {
                    return Err("Provide an exact issue:123 or pull:123 thread key.".into());
                }
                command = Command::UnlinkSourceThread {
                    source_id:source_id.clone(),ticket_id:ticket_id.clone(),
                    subject_key:subject_key.clone(),
                };
                ops += 1;
                index += 3;
            }
            "--inspect-ticket-effects" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--inspect-ticket-effects needs a Ticket ID.\n\n{}", help()))?;
                validate_ticket_id(value)?;
                let mut before_created_at = None;
                let mut before_key = None;
                let mut unresolved_only = false;
                index += 1;
                while let Some(flag) = args.get(index + 1) {
                    match flag.as_str() {
                        "--unresolved-only" if !unresolved_only => {
                            unresolved_only = true;
                            index += 1;
                        }
                        "--before" if before_created_at.is_none() => {
                            let timestamp = args.get(index + 2)
                                .ok_or("--before requires a positive millisecond timestamp and effect key.")?;
                            let effect_key = args.get(index + 3)
                                .ok_or("--before requires the previous page's last effect key.")?;
                            let created_at = timestamp.parse::<i64>()
                                .map_err(|_| "Provide a positive effect receipt timestamp.")?;
                            if created_at <= 0 {
                                return Err("Provide a positive effect receipt timestamp.".into());
                            }
                            validate_effect_key(effect_key)?;
                            before_created_at = Some(created_at);
                            before_key = Some(effect_key.clone());
                            index += 3;
                        }
                        _ => break,
                    }
                }
                command = Command::InspectTicketEffects {
                    ticket_id: value.clone(), before_created_at, before_key, unresolved_only,
                };
                ops += 1;
            }
            "--inspect-ticket-graph" => {
                let ticket_id = args.get(index + 1)
                    .ok_or_else(|| format!("--inspect-ticket-graph needs a Ticket ID.\n\n{}", help()))?;
                validate_ticket_id(ticket_id)?;
                command = Command::InspectTicketGraph { ticket_id: ticket_id.clone() };
                ops += 1;
                index += 1;
            }
            "--inspect-ticket" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--inspect-ticket needs a ticket ID.\n\n{}", help()))?;
                validate_ticket_id(value)?;
                command = Command::InspectTicket { ticket_id: value.clone() };
                ops += 1;
                index += 1;
            }
            "--accept-ticket" => {
                let ticket_id = args.get(index + 1)
                    .ok_or_else(|| format!("--accept-ticket needs a ticket ID.\n\n{}", help()))?;
                validate_ticket_id(ticket_id)?;
                let revision = args.get(index + 2)
                    .ok_or_else(|| format!("--accept-ticket needs an expected revision.\n\n{}", help()))?;
                let expected_revision = parse_ticket_revision(revision)?;
                command = Command::AcceptTicket { ticket_id: ticket_id.clone(), expected_revision };
                ops += 1;
                index += 2;
            }
            "--reject-ticket" => {
                let ticket_id = args.get(index + 1)
                    .ok_or_else(|| format!("--reject-ticket needs a ticket ID.\n\n{}", help()))?;
                validate_ticket_id(ticket_id)?;
                let revision = args.get(index + 2)
                    .ok_or_else(|| format!("--reject-ticket needs an expected revision.\n\n{}", help()))?;
                let expected_revision = parse_ticket_revision(revision)?;
                let reason = args.get(index + 3)
                    .ok_or_else(|| format!("--reject-ticket needs a reason.\n\n{}", help()))?;
                if reason.trim().is_empty() || reason.starts_with("--") {
                    return Err("Provide a nonempty rejection reason.".into());
                }
                command = Command::RejectTicket {
                    ticket_id: ticket_id.clone(), expected_revision, reason: reason.clone(),
                };
                ops += 1;
                index += 3;
            }
            "--disable-orchestration" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or_else(|| format!("--disable-orchestration needs an orchestration ID.\n\n{}", help()))?;
                validate_orchestration_id(orchestration_id)?;
                let revision = args.get(index + 2)
                    .ok_or_else(|| format!("--disable-orchestration needs an expected revision.\n\n{}", help()))?;
                command = Command::DisableOrchestration {
                    orchestration_id: orchestration_id.clone(),
                    expected_revision: parse_team_revision(revision)?,
                };
                ops += 1;
                index += 2;
            }
            "--enable-orchestration" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--enable-orchestration needs a Team ID and policy revision.")?;
                validate_orchestration_id(orchestration_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--enable-orchestration needs an expected policy revision.")?;
                command = Command::EnableOrchestration {
                    orchestration_id: orchestration_id.clone(),
                    expected_revision: parse_team_revision(revision)?,
                };
                ops += 1;
                index += 2;
            }
            "--duplicate-orchestration" => {
                let source_id = args.get(index + 1)
                    .ok_or("--duplicate-orchestration needs a source Team ID, policy revision and new name.")?;
                validate_orchestration_id(source_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--duplicate-orchestration needs an expected source policy revision.")?;
                let name = args.get(index + 3)
                    .ok_or("--duplicate-orchestration needs a quoted new Team name.")?;
                if name.is_empty() || name.trim() != name || name.starts_with("--")
                    || name.len()>120 || name.chars().any(char::is_control) {
                    return Err("Provide a bounded Team name (quote names with spaces).".into());
                }
                command = Command::DuplicateOrchestration {
                    source_id: source_id.clone(),
                    expected_revision: parse_team_revision(revision)?,
                    name: name.clone(),
                };
                ops += 1;
                index += 3;
            }
            "--rename-orchestration" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or_else(|| format!("--rename-orchestration needs a Team ID, revision and new name.\\n\\n{}", help()))?;
                validate_orchestration_id(orchestration_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--rename-orchestration needs an expected policy revision.")?;
                let name = args.get(index + 3)
                    .ok_or("--rename-orchestration needs a quoted new Team name.")?;
                if name.is_empty() || name.trim() != name || name.starts_with("--")
                    || name.len()>120 || name.chars().any(char::is_control) {
                    return Err("Provide a bounded Team name (quote names with spaces).".into());
                }
                command = Command::RenameOrchestration {
                    orchestration_id: orchestration_id.clone(),
                    expected_revision: parse_team_revision(revision)?, name: name.clone(),
                };
                ops += 1;
                index += 3;
            }
            "--set-team-roster" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--set-team-roster needs Team ID, policy revision and roster file.")?;
                validate_orchestration_id(orchestration_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--set-team-roster needs an expected policy revision.")?;
                let roster_file = args.get(index + 3)
                    .ok_or("--set-team-roster needs a local JSON roster file.")?;
                if roster_file.is_empty() || roster_file.starts_with("--") {
                    return Err("Provide a local Team roster file path, not another command.".into());
                }
                command = Command::SetTeamRoster {
                    orchestration_id: orchestration_id.clone(),
                    expected_revision: parse_team_revision(revision)?,
                    roster_file: PathBuf::from(roster_file),
                };
                ops += 1;
                index += 3;
            }
            "--set-team-attempt-limit" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--set-team-attempt-limit needs Team ID, policy revision and limit.")?;
                validate_orchestration_id(orchestration_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--set-team-attempt-limit needs an expected policy revision.")?;
                let limit = args.get(index + 3)
                    .and_then(|value| value.parse::<i64>().ok())
                    .filter(|value| (1..=4).contains(value))
                    .ok_or("Worker attempt limit must be between 1 and 4.")?;
                command = Command::SetTeamAttemptLimit {
                    orchestration_id: orchestration_id.clone(),
                    expected_revision: parse_team_revision(revision)?,
                    limit,
                };
                ops += 1;
                index += 3;
            }
            "--set-team-strategy" => {
                let orchestration_id = args.get(index + 1)
                    .ok_or("--set-team-strategy needs Team ID, revision, strategy and purpose file.")?;
                validate_orchestration_id(orchestration_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--set-team-strategy needs an expected policy revision.")?;
                let strategy = args.get(index + 3)
                    .ok_or("--set-team-strategy needs a strategy name.")?;
                if !matches!(strategy.as_str(),
                    "balanced" | "economy" | "performance" | "security_critical" | "custom") {
                    return Err("Unsupported Team optimization strategy.".into());
                }
                let purpose_file = args.get(index + 4)
                    .ok_or("--set-team-strategy needs a local UTF-8 purpose file.")?;
                if purpose_file.is_empty() || purpose_file.starts_with("--") {
                    return Err("Provide a local purpose file path, not another command.".into());
                }
                command = Command::SetTeamStrategy {
                    orchestration_id: orchestration_id.clone(),
                    expected_revision: parse_team_revision(revision)?,
                    strategy: strategy.clone(), purpose_file: PathBuf::from(purpose_file),
                };
                ops += 1;
                index += 4;
            }
            "--quarantine-detached-ticket" => {
                let ticket_id=args.get(index+1)
                    .ok_or("--quarantine-detached-ticket needs Ticket ID and revision.")?;
                validate_ticket_id(ticket_id)?;
                let revision=args.get(index+2)
                    .ok_or("--quarantine-detached-ticket needs a Ticket revision.")?;
                command=Command::QuarantineDetachedTicket {
                    ticket_id:ticket_id.clone(),expected_revision:parse_ticket_revision(revision)?,
                };
                ops+=1;
                index+=2;
            }
            "--quarantine-expired-ticket" => {
                let ticket_id = args.get(index + 1)
                    .ok_or("--quarantine-expired-ticket needs Ticket ID and revision.")?;
                validate_ticket_id(ticket_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--quarantine-expired-ticket needs a Ticket revision.")?;
                command = Command::QuarantineExpiredTicket {
                    ticket_id: ticket_id.clone(), expected_revision: parse_ticket_revision(revision)?,
                };
                ops += 1;
                index += 2;
            }
            "--quarantine-pending-effect" => {
                let ticket_id = args.get(index + 1)
                    .ok_or("--quarantine-pending-effect needs a Ticket ID, revision, effect key and SHA-256.")?;
                validate_ticket_id(ticket_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--quarantine-pending-effect needs an expected Ticket revision.")?;
                let effect_key = args.get(index + 3)
                    .ok_or("--quarantine-pending-effect needs the exact effect key.")?;
                validate_effect_key(effect_key)?;
                let payload_sha256 = args.get(index + 4)
                    .ok_or("--quarantine-pending-effect needs the exact payload SHA-256.")?;
                if payload_sha256.len() != 64
                    || !payload_sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                {
                    return Err("Provide the exact lowercase SHA-256 from the receipt.".into());
                }
                command = Command::QuarantinePendingEffect {
                    ticket_id: ticket_id.clone(),
                    expected_revision: parse_ticket_revision(revision)?,
                    effect_key: effect_key.clone(),
                    payload_sha256: payload_sha256.clone(),
                };
                ops += 1;
                index += 4;
            }
            "--quarantine-orphan-ticket" => {
                let ticket_id=args.get(index+1)
                    .ok_or("--quarantine-orphan-ticket needs Ticket ID and revision.")?;
                validate_ticket_id(ticket_id)?;
                let revision=args.get(index+2)
                    .ok_or("--quarantine-orphan-ticket needs a Ticket revision.")?;
                command=Command::QuarantineOrphanRunning {
                    ticket_id:ticket_id.clone(),
                    expected_revision:parse_ticket_revision(revision)?,
                };
                ops+=1;
                index+=2;
            }
            "--quarantine-terminal-orphan" => {
                let ticket_id = args.get(index + 1)
                    .ok_or("--quarantine-terminal-orphan needs Ticket ID and revision.")?;
                validate_ticket_id(ticket_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--quarantine-terminal-orphan needs a Ticket revision.")?;
                command = Command::QuarantineTerminalOrphan {
                    ticket_id: ticket_id.clone(),
                    expected_revision: parse_ticket_revision(revision)?,
                };
                ops += 1;
                index += 2;
            }
            "--stop-ticket-at-revision" => {
                let ticket_id = args.get(index + 1)
                    .ok_or("--stop-ticket-at-revision needs a Ticket ID and expected revision.")?;
                validate_ticket_id(ticket_id)?;
                let revision = args.get(index + 2)
                    .ok_or("--stop-ticket-at-revision needs an expected Ticket revision.")?;
                command = Command::StopTicketAtRevision {
                    ticket_id: ticket_id.clone(),
                    expected_revision: parse_ticket_revision(revision)?,
                };
                ops += 1;
                index += 2;
            }
            "--stop-ticket" => {
                let ticket_id = args.get(index + 1)
                    .ok_or_else(|| format!("--stop-ticket needs a ticket ID.\n\n{}", help()))?;
                validate_ticket_id(ticket_id)?;
                command = Command::StopTicket { ticket_id: ticket_id.clone() };
                ops += 1;
                index += 1;
            }
            "--sync-bots" => {
                let value = args.get(index + 1)
                    .ok_or_else(|| format!("--sync-bots needs a source registry path.\n\n{}", help()))?;
                command = Command::SyncBots { path: PathBuf::from(value) };
                ops += 1;
                index += 1;
            }
            "--import-bots" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("--import-bots needs a file path.\n\n{}", help()))?;
                import = Some(PathBuf::from(value));
                index += 1;
            }
            "--device-name" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("--device-name needs a name.\n\n{}", help()))?;
                name = Some(value.clone());
                index += 1;
            }
            other => return Err(format!("Unknown flag `{other}`.\n\n{}", help())),
        }
        index += 1;
    }
    if ops > 1 {
        return Err(format!(
            "Choose one command; Ticket actions cannot be combined with serving or other commands.\n\n{}",
            help()
        ));
    }
    if matches!(command, Command::SyncBots { .. }) && name.is_some() {
        return Err(format!("--device-name only works with --sign-in.\n\n{}", help()));
    }
    if name.is_some() && !sign_in {
        return Err(format!("--device-name only works with --sign-in.\n\n{}", help()));
    }
    // Built at the end, so flags work in any order.
    if sign_in {
        command = Command::SignIn { name };
    }
    // An import composes only with serving (or a first-run sign-in): the exiting ops
    // would silently ignore the file.
    if import.is_some() && !matches!(command, Command::Serve | Command::SignIn { .. }) {
        return Err(format!("--import-bots only works with serve or --sign-in.\n\n{}", help()));
    }
    Ok((command, import))
}

fn validate_ticket_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128
        || !value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err("Provide a bounded alphanumeric ticket ID.".into());
    }
    Ok(())
}

fn validate_member_identity(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128
        || !value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err("Provide a bounded alphanumeric bot or assignment ID.".into());
    }
    Ok(())
}

fn validate_orchestration_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128
        || !value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err("Provide a bounded alphanumeric orchestration ID.".into());
    }
    Ok(())
}

fn validate_project_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128
        || !value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err("Provide a bounded exact native Project ID.".into());
    }
    Ok(())
}

fn validate_effect_key(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 128
        || !value.bytes().all(|c| c.is_ascii_alphanumeric()
            || matches!(c, b'-' | b'_' | b'.' | b':'))
    {
        return Err("Provide the bounded effect key from nextCursor.".into());
    }
    Ok(())
}

/// Team policy revisions start at 1; Ticket revisions intentionally start at 0.
fn parse_team_revision(value: &str) -> Result<i64, String> {
    value.parse::<i64>().ok().filter(|revision| *revision > 0)
        .ok_or_else(|| "Provide a positive expected Team policy revision.".into())
}

fn parse_ticket_revision(value: &str) -> Result<i64, String> {
    let revision = value.parse::<i64>()
        .map_err(|_| "Provide a nonnegative expected Ticket revision.".to_string())?;
    if revision < 0 {
        return Err("Provide a nonnegative expected Ticket revision.".into());
    }
    Ok(revision)
}

fn help() -> String {
    "PandaCode headless host (docs/headless.md)

Usage: pandacode serve [--sign-in [--device-name <name>]] [--sign-out] [--full-access | --no-full-access] [--export-bots <file>] [--import-bots <file>] [--sync-bots <sources.json>] [--import-orchestrations <file> | --export-orchestrations <file>] [--create-orchestration <json-file> | --inspect-orchestrations | --list-tickets | --list-tickets-page [--before <created-ms> <ticket-id>] | --list-ticket-activity <id> [--before <audit-id>] | --list-team-member-activity <team-id> <bot-id> [--before <created-ms> <assignment-id>] | --list-review-inbox [--before <ms> <ticket-id>] | --list-team-review-inbox <team-id> [--before <ms> <ticket-id>] | --list-recovery-inbox [--before <priority> <ms> <ticket-id>] | --list-team-unresolved-effects <team-id> [--before <ms> <ticket-id> <effect-key>] | --list-demands <state> [--before <ms> <ticket-id>] [--search <fragment>] [--project <id>] | --list-team-demands <id> <state> [--before <ms> <ticket-id>] [--search <fragment>] [--project <id>] | --inspect-team-readiness <id> | --preflight-demand <team-id> <direct|coordinated> <objective-file> | --preflight-worker-demand <team-id> <worker-bot-id> <objective-file> | --inspect-team-roster <id> | --inspect-source-health <id> | --set-source-mode <id> <revision> <off|observe> | --observe-source <id> | --observe-team-sources <team-id> | --observe-check-run <id> <check-run-id> | --observe-linked-pr-checks <source-id> <ticket-id> <pull:123> | --queue-demand <team-id> <request-key> <objective-file> | --queue-worker-demand <team-id> <worker-bot-id> <request-key> <objective-file> | --inspect-ticket <id> | --inspect-ticket-graph <id> | --inspect-ticket-effects <id> [--before <ms> <effect-key>] [--unresolved-only] | --inspect-ticket-source <id> | --link-source-thread <source-id> <ticket-id> <issue:123|pull:123> | --link-issue-pull <source-id> <ticket-id> <issue:123> <pull:123> | --verify-link-issue-pull <source-id> <ticket-id> <issue:123> <pull:123> | --unlink-source-thread <source-id> <ticket-id> <issue:123|pull:123> | --accept-ticket <id> <revision> | --reject-ticket <id> <revision> <reason> | --stop-ticket <id> | --stop-ticket-at-revision <id> <revision> | --quarantine-detached-ticket <id> <revision> | --quarantine-expired-ticket <id> <revision> | --quarantine-pending-effect <id> <revision> <effect-key> <payload-sha256> | --quarantine-terminal-orphan <id> <revision> | --disable-orchestration <id> <revision> | --enable-orchestration <id> <revision> | --rename-orchestration <id> <revision> <name> | --duplicate-orchestration <id> <revision> <name> | --set-team-strategy <id> <revision> <strategy> <purpose-file>]

Runs this computer as an always-on PandaCode Connect host: your devices see it as a
computer and can start, watch, steer and approve tasks on it. Linux servers run it
under `xvfb-run -a pandacode serve …` (a display is required to start, none is used).

  --sign-in              First run: sign in with the GitHub device flow, enable hosting, then serve.
  --sign-out             Forget the Connect sign-in on this server and exit.
  --full-access          Let remote devices pick Full access (no approvals); exits after saving.
  --no-full-access       Require approvals again (the default); exits after saving.
  --device-name <name>   Name shown to your devices when signing in (default: the hostname).
  --import-bots <file>   Create/update bots, channels, schedules and rules from a JSON file
                         (docs/headless.md), then serve. Re-applying a file converges.
  --export-bots <file>   Write every bot's configuration (no secrets) to a JSON file and exit.
  --sync-bots <file>     Fetch pinned GitHub manifests from a source registry, back up,
                         import once, report results and exit (does not start serving).
  --preview-orchestrations <file>
                         Validate and preview disabled presets without importing or backing up.
  --import-orchestrations <file>
                         Explicitly import disabled SuperAgent presets after native validation
                         and a verified config-only backup; exit without starting tasks.
  --export-orchestrations <file>
                         Export portable SuperAgent presets without tickets or secrets; exit.
  --create-orchestration <json-file>
                          Create one disabled Team with ordered Workers, optional
                          Reviewers and future-task policy from bounded local JSON.
                          No provider, task grant, GitHub Auto or effect is started.
  --inspect-orchestrations
                         List native orchestration status without modifying live tasks; exit.
  --list-tickets         Legacy first 200 redacted native SuperAgent Tickets (JSON array).
  --list-tickets-page [--before <created-ms> <ticket-id>]
                         Inspect every Ticket through stable creation-order pages
                         (200 per page, JSON nextCursor). Read-only; no recovery.
  --list-ticket-activity <id> [--before <audit-id>]
                          Page 100 redacted native Ticket audit events by immutable ID.
                          Event details remain private; never replay or recover; exit.
  --list-team-member-activity <team-id> <bot-id> [--before <created-ms> <assignment-id>]
                          Page native Team-scoped assignments without recovery or provider
                          dispatch; use nextCursor to inspect additional history.
  --list-review-inbox [--before <ms> <ticket-id>]
                         Page pending human/Council decisions across Teams using
                         nextCursor; redact objectives; never approve; exit.
  --list-team-review-inbox <team-id> [--before <ms> <ticket-id>]
                          Page native pending decisions for one Team only. No Council
                          verdict, provider launch, recovery, task grant or approval.
  --list-recovery-inbox [--before <priority> <ms> <ticket-id>]
                         Page native recovery/decision Tickets across Teams by stable priority
                         then update time and ID; no provider or remote replay; exit.
  --list-team-recovery-inbox <team-id> [--before <priority> <ms> <ticket-id>]
                         Page the same native recovery/decision Tickets for one Team,
                         scoped before cursor validation and pagination. No replay; exit.
  --list-unresolved-effects [--before <ms> <ticket-id> <effect-key>]
                         Page unresolved native effect receipts across all Tickets,
                         including older ones; never retry or acknowledge remotely; exit.
  --list-demands <state> [--before <ms> <ticket-id>] [--search <fragment>] [--project <id>]
                         Filter by each Ticket's admitted native Project before paging;
                         search/project flags may follow --before in either order; redacted.
  --list-team-unresolved-effects <team-id> [--before <ms> <ticket-id> <effect-key>]
                         Page the read-only, revisioned pending/uncertain effect receipts
                         belonging to one native Team. No provider call or replay.
  --list-team-demands <id> <state> [--before <ms> <ticket-id>] [--search <fragment>] [--project <id>]
                         Page up to 200 native Demands for one Team and exact Ticket state
                         (or all). Project filters use admitted Tickets, not current Teams;
                         nextCursor pages without exposing evidence or starting recovery; exit.
  --inspect-team-roster <id>
                         Inspect ordered native Team members, configured models, harnesses and
                         assigned Skill IDs without provider admission or Library trust scanning.
  --inspect-team-readiness <id>
                         Inspect a Team's read-only native manual admission preflight;
                         live sessions and policy must be rechecked on actual admission.
  --preflight-demand <team-id> <direct|coordinated> <objective-file>
                         Validate the exact bounded UTF-8 objective against native manual policy.
                         Prints no objective, issues no Mandate, creates no Ticket, launches no provider.
                         Detached occupancy is unknown; actual admission must recheck.
  --preflight-worker-demand <team-id> <worker-bot-id> <objective-file>
                         Check one configured Worker against native multi-member policy.
                         Read-only; no Ticket, Mandate or provider launch.
                         Detached occupancy is unknown; actual admission must recheck.
  --inspect-source-health <id>
                         Inspect native source cursor, stored receipts and policy without
                         GitHub access, writes or restart recovery; exit.
  --inspect-source-decisions <id> [--page | --before <sequence> <delivery-id>]
                          Read 50 source-scoped decisions. Legacy command returns an array;
                          --page returns nextCursor, --before follows the exact keyset.
                          No GitHub polling, startup recovery, worker admission or writes; exit.
  --create-source <team-id> <owner/repo> <actor> <event-kind>
                         Register an exact GitHub source in Off through native Team storage;
                         repeat safely without enabling an existing source.
  --list-team-sources <team-id>
                         Read the Team's native source policies/cursors; no GitHub request.
  --set-source-mode <id> <revision> <off|observe>
                         Enable or immediately revoke a native GitHub Observe source with
                         optimistic policy revision; no GitHub call or task execution; exit.
  --observe-source <id> Authenticated one-shot GitHub Observe poll for an activated source; exit.
                         No Auto, inferred task, webhook or external write.
  --observe-team-sources <team-id>
                         Observe every enabled GitHub Events source of a Team once.
                         Off and Checks-only sources are skipped without requests;
                         failures exit nonzero; earlier receipts remain durable.
  --observe-check-run <id> <check-run-id>
                         Verify one completed GitHub Checks API result for an Observe source;
                         record a durable receipt, never approve a Ticket or invoke Auto.
  --observe-linked-pr-checks <source-id> <ticket-id> <pull:123>
                         Read at most 100 authenticated Checks for the explicitly linked
                         PR head, store deduplicated Observe receipts and skip pending runs.
                         Incomplete/ambiguous windows fail; this never approves work.
  --queue-demand <team-id> <request-key> <objective-file>
                         Persist a blocked, read-only manual Ticket from a local UTF-8 file
                         (max 16 KiB). Does not grant tools or dispatch a provider.
  --queue-worker-demand <team-id> <worker-bot-id> <request-key> <objective-file>
                         Stage a named configured Team Worker without choosing the
                         first Worker implicitly. No provider/tool grant or launch;
                         native Desktop retry independently rechecks the Ticket.
  --inspect-ticket <id>  Inspect redacted native Ticket/mandate/attempt metadata; exit.
  --inspect-ticket-graph <id>
                         Inspect bounded native Ticket/attempt topology without task
                         objectives, results, inferred subagents or Council verdicts; exit.
  --inspect-ticket-effects <id> [--before <ms> <effect-key>] [--unresolved-only]
                         Page 100 native effect receipt keys, payload digests and
                         states, optionally unresolved only; never replay or mark applied; exit.
  --inspect-ticket-source <id> [--page | --before <ms> <source-id> <delivery-id>]
                          Read 50 linked receipts with native keyset pagination;
                          --page returns nextCursor, --before continues from that cursor.
                          No GitHub polling, task execution or startup recovery; exit.
  --list-ticket-source-links <ticket-id>
                         Inspect exact native source/thread associations for one Ticket,
                         including links without events. No provider, GitHub call,
                         task authority or startup recovery; exit.
  --link-source-thread <source-id> <ticket-id> <issue:123|pull:123>
                          Add an exact operator-asserted source/Ticket context link through
                          native policy; no provider, grant, GitHub call or task launch.
  --link-issue-pull <source-id> <ticket-id> <issue:123> <pull:123>
                         Explicitly assert that an existing Ticket's linked Issue is related to
                         a PR in the same Team/repository; never authenticate a GitHub relationship,
                         grant authority or run a provider.
  --verify-link-issue-pull <source-id> <ticket-id> <issue:123> <pull:123>
                         Verify GitHub Issue/PR object identity and base repository
                         using native gh, then link operator-asserted context.
                         Neither a relationship nor task execution is authorized.
  --unlink-source-thread <source-id> <ticket-id> <issue:123|pull:123>
                         Revoke only this explicit Ticket/GitHub context link, retain immutable
                         receipts and audit, without starting a worker or provider; exit.
  --accept-ticket <id> <revision>
                         Accept current worker evidence at the expected Ticket revision; exit.
  --reject-ticket <id> <revision> <reason>
                         Reject current worker evidence with a required reason; quote it if it has spaces.
  --stop-ticket <id>    Emergency revoke of a Ticket mandate without a prior revision.
  --stop-ticket-at-revision <id> <revision>
                         Revoke only the observed Ticket revision, with no change on mismatch.
  --quarantine-detached-ticket <id> <revision>
  --quarantine-pending-effect <id> <revision> <effect-key> <payload-sha256>
                         Interrupt a running Ticket whose sole active Assignment has a
                         missing provider session; never replay effects.
  --quarantine-expired-ticket <id> <revision>
                         Revoke one expired or invalid-clock running native assignment;
                         the live server handles provider cancellation. No replay.
  --quarantine-orphan-ticket <id> <revision>
                          Quarantine a structurally orphaned running Ticket;
                          no provider or external-effect replay.
  --quarantine-terminal-orphan <id> <revision>
                         Quarantine orphaned native attempts on terminal Tickets;
                         do not launch providers or replay external effects.
  --disable-orchestration <id> <revision>
                         Turn a Team Off and atomically revoke its mandates;
                         live provider calls may finish but native authority stays revoked.
  --enable-orchestration <id> <revision>
                         Explicitly enable a valid Team for future manual Demands;
                         no provider, Ticket or grant starts automatically.
  --duplicate-orchestration <id> <revision> <new-name>
                         Clone native roles, retry policy and strategy into an Off Team.
                         Never copy Tickets, source subscriptions or task grants.
  --rename-orchestration <id> <revision> <name>
                         Explicit local, revision-guarded Team rename only while disabled
                         and all Tickets/effects are settled; does not change bots, roles
                         or task authorization. Quote names with spaces.
  --set-team-roster <id> <revision> <roster-json-file>
                         Replace the disabled Team's ordered Workers and optional
                         Reviewers through the desktop-equivalent native CAS;
                         no provider launch, new bot or grant. Max 16 KiB JSON.
  --set-team-attempt-limit <id> <revision> <1..4>
                         Set future Worker retry budget on a disabled, settled Team;
                         does not retry an existing Demand.
  --set-team-strategy <id> <revision> <strategy> <purpose-file>
                         Set future Demand strategy and purpose using native revision guards.
                         The Team must be disabled and all Tickets/effects settled.
                         The UTF-8 purpose file is local and limited to 400 characters;
                         no bot permissions or model selection are changed."
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(flags: &str) -> Vec<String> {
        flags.split_whitespace().map(str::to_owned).collect()
    }

    /// The command half of a parse, for tests that don't care about the import path.
    fn parsed(flags: &str) -> Result<Command, String> {
        parse(&argv(flags)).map(|(command, _)| command)
    }

    #[test]
    fn flags_parse_in_any_order() {
        assert_eq!(parsed(""), Ok(Command::Serve));
        assert_eq!(parsed("--sign-in"), Ok(Command::SignIn { name: None }));
        assert_eq!(
            parsed("--sign-in --device-name build-server"),
            Ok(Command::SignIn { name: Some("build-server".into()) })
        );
        assert_eq!(
            parsed("--device-name build-server --sign-in"),
            Ok(Command::SignIn { name: Some("build-server".into()) })
        );
        assert_eq!(parsed("--full-access"), Ok(Command::FullAccess { enabled: true }));
        assert_eq!(parsed("--no-full-access"), Ok(Command::FullAccess { enabled: false }));
        assert_eq!(parsed("--sign-out"), Ok(Command::SignOut));
        assert_eq!(
            parsed("--export-bots bots.json"),
            Ok(Command::ExportBots { path: PathBuf::from("bots.json") })
        );
        assert_eq!(parsed("--preview-orchestrations presets.json"),
            Ok(Command::PreviewOrchestrations { path: PathBuf::from("presets.json") }));
        assert!(parsed("--preview-orchestrations").is_err());
        assert!(parsed("--preview-orchestrations p.json --import-orchestrations p.json").is_err());
        assert_eq!(parsed("--import-orchestrations presets.json"),
            Ok(Command::ImportOrchestrations { path: PathBuf::from("presets.json") }));
        assert_eq!(parsed("--export-orchestrations presets.json"),
            Ok(Command::ExportOrchestrations { path: PathBuf::from("presets.json") }));
        assert_eq!(parsed("--inspect-orchestrations"), Ok(Command::InspectOrchestrations));
        assert_eq!(parsed("--duplicate-orchestration team_123 2 Copy"),
            Ok(Command::DuplicateOrchestration {
                source_id: "team_123".into(), expected_revision: 2,
                name: "Copy".into(),
            }));
        for invalid in [
            "--duplicate-orchestration",
            "--duplicate-orchestration team_123",
            "--duplicate-orchestration team_123 2",
            "--duplicate-orchestration ../unsafe 2 Copy",
            "--duplicate-orchestration team_123 nope Copy",
            "--duplicate-orchestration team_123 0 Copy",
            "--duplicate-orchestration team_123 2 --stop-ticket",
            "--duplicate-orchestration team_123 2 Copy --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--create-orchestration team.json"),
            Ok(Command::CreateOrchestration { config_file: PathBuf::from("team.json") }));
        for invalid in [
            "--create-orchestration",
            "--create-orchestration --stop-ticket",
            "--create-orchestration team.json --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        let valid = r#"{
            "config":{"name":"Safe Team","coordinatorBotId":"lead","workerBotId":"worker"},
            "roster":{"workerBotIds":["worker","worker-two"],"reviewerBotIds":[]},
            "maxWorkerAttempts":3,"purpose":"Headless delivery","strategy":"balanced"
        }"#;
        let config: HeadlessTeamCreateDocument = serde_json::from_str(valid).unwrap();
        assert_eq!(config.roster.worker_bot_ids.len(), 2);
        assert_eq!(config.max_worker_attempts, Some(3));
        for unsafe_json in [
            r#"{"config":{"name":"Team","coordinatorBotId":"lead","workerBotId":"worker"},
                "roster":{"workerBotIds":["worker"],"reviewerBotIds":[]},"enabled":true}"#,
            r#"{"config":{"name":"Team","coordinatorBotId":"lead","workerBotId":"worker",
                "allowedSideEffects":true},"roster":{"workerBotIds":["worker"],"reviewerBotIds":[]}}"#,
            r#"{"config":{"name":"Team","coordinatorBotId":"lead","workerBotId":"worker"},
                "roster":{"workerBotIds":["worker"],"reviewerBotIds":[],"grantIssued":true}}"#,
        ] {
            assert!(serde_json::from_str::<HeadlessTeamCreateDocument>(unsafe_json).is_err());
        }
        assert_eq!(parsed("--list-tickets"), Ok(Command::ListTickets));
        assert_eq!(parsed("--list-tickets-page"), Ok(Command::ListTicketsPage {
            before_created_at: None, before_id: None,
        }));
        assert_eq!(parsed("--list-tickets-page --before 123 ticket_3"), Ok(Command::ListTicketsPage {
            before_created_at: Some(123), before_id: Some("ticket_3".into()),
        }));
        for invalid in [
            "--list-tickets-page --before", "--list-tickets-page --before 123",
            "--list-tickets-page --before 0 ticket_3",
            "--list-tickets-page --before -1 ticket_3",
            "--list-tickets-page --before nope ticket_3",
            "--list-tickets-page --before 123 ../escape",
            "--list-tickets-page --stop-ticket ticket_3",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--list-ticket-activity ticket_123"), Ok(Command::ListTicketActivity {
            ticket_id:"ticket_123".into(), before_id:None,
        }));
        assert_eq!(parsed("--list-ticket-activity ticket_123 --before 42"), Ok(Command::ListTicketActivity {
            ticket_id:"ticket_123".into(), before_id:Some(42),
        }));
        for invalid in [
            "--list-ticket-activity", "--list-ticket-activity ../unsafe",
            "--list-ticket-activity ticket_123 --before",
            "--list-ticket-activity ticket_123 --before 0",
            "--list-ticket-activity ticket_123 --before -1",
            "--list-ticket-activity ticket_123 --before invalid",
            "--list-ticket-activity ticket_123 --before 9223372036854775808",
            "--list-ticket-activity ticket_123 --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }

        assert_eq!(parsed("--list-team-member-activity team_123 bot_1"),
            Ok(Command::ListTeamMemberActivity {
                orchestration_id: "team_123".into(), bot_id: "bot_1".into(),
                before_created_at: None, before_assignment_id: None,
            }));
        assert_eq!(parsed("--list-team-member-activity team_123 bot_1 --before 123 assignment_4"),
            Ok(Command::ListTeamMemberActivity {
                orchestration_id: "team_123".into(), bot_id: "bot_1".into(),
                before_created_at: Some(123), before_assignment_id: Some("assignment_4".into()),
            }));
        for invalid in [
            "--list-team-member-activity", "--list-team-member-activity team_123",
            "--list-team-member-activity ../unsafe bot_1",
            "--list-team-member-activity team_123 ../private",
            "--list-team-member-activity team_123 bot_1 --before",
            "--list-team-member-activity team_123 bot_1 --before 0 assignment_4",
            "--list-team-member-activity team_123 bot_1 --before 123",
            "--list-team-member-activity team_123 bot_1 --before 123 ../escape",
            "--list-team-member-activity team_123 bot_1 --stop-ticket ticket_1",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }

        assert_eq!(parsed("--list-review-inbox"), Ok(Command::ListReviewInbox {
            before_updated_at: None, before_id: None,
        }));
        assert_eq!(parsed("--list-review-inbox --before 12345 review_001"),
            Ok(Command::ListReviewInbox {
                before_updated_at: Some(12345), before_id: Some("review_001".into()),
            }));
        for invalid in [
            "--list-review-inbox --before",
            "--list-review-inbox --before 0 review_001",
            "--list-review-inbox --before -1 review_001",
            "--list-review-inbox --before 12345 ../private",
            "--list-review-inbox --before 12345",
            "--list-review-inbox --before 12345 review_001 --stop-ticket review_001",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--list-team-review-inbox team_123"),
            Ok(Command::ListTeamReviewInbox {
                orchestration_id:"team_123".into(),
                before_updated_at:None, before_id:None,
            }));
        assert_eq!(parsed("--list-team-review-inbox team_123 --before 12345 review_001"),
            Ok(Command::ListTeamReviewInbox {
                orchestration_id:"team_123".into(),
                before_updated_at:Some(12345), before_id:Some("review_001".into()),
            }));
        for invalid in [
            "--list-team-review-inbox",
            "--list-team-review-inbox ../unsafe",
            "--list-team-review-inbox team_123 --before",
            "--list-team-review-inbox team_123 --before 0 review_001",
            "--list-team-review-inbox team_123 --before 12345 ../private",
            "--list-team-review-inbox team_123 --before 12345",
            "--list-team-review-inbox team_123 --stop-ticket review_001",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--list-recovery-inbox"), Ok(Command::ListRecoveryInbox {
            before_priority:None,before_updated_at:None,before_id:None
        }));
        assert_eq!(parsed("--list-recovery-inbox --before 1 12345 ticket_1"),
            Ok(Command::ListRecoveryInbox {
                before_priority:Some(1),before_updated_at:Some(12345),before_id:Some("ticket_1".into())
            }));
        for invalid in [
            "--list-recovery-inbox --before",
            "--list-recovery-inbox --before 3 12345 ticket_1",
            "--list-recovery-inbox --before 1 0 ticket_1",
            "--list-recovery-inbox --before 1 12345 ../private",
        ] { assert!(parsed(invalid).is_err(),"{invalid}"); }
        assert!(parsed("--list-recovery-inbox --stop-ticket ticket_1").is_err());
        assert_eq!(parsed("--list-team-recovery-inbox team_123"),
            Ok(Command::ListTeamRecoveryInbox {
                orchestration_id:"team_123".into(),
                before_priority:None,before_updated_at:None,before_id:None,
            }));
        assert_eq!(parsed("--list-team-recovery-inbox team_123 --before 2 12345 ticket_1"),
            Ok(Command::ListTeamRecoveryInbox {
                orchestration_id:"team_123".into(),
                before_priority:Some(2),before_updated_at:Some(12345),
                before_id:Some("ticket_1".into()),
            }));
        for invalid in [
            "--list-team-recovery-inbox",
            "--list-team-recovery-inbox ../unsafe",
            "--list-team-recovery-inbox team_123 --before",
            "--list-team-recovery-inbox team_123 --before 3 12345 ticket_1",
            "--list-team-recovery-inbox team_123 --before 1 0 ticket_1",
            "--list-team-recovery-inbox team_123 --before 1 12345 ../private",
            "--list-team-recovery-inbox team_123 --stop-ticket ticket_1",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--list-unresolved-effects"), Ok(Command::ListUnresolvedEffects {
            before_updated_at:None,before_ticket_id:None,before_effect_key:None,
        }));
        assert_eq!(parsed("--list-unresolved-effects --before 1000 ticket_1 github:issue:42"),
            Ok(Command::ListUnresolvedEffects {
                before_updated_at:Some(1000),before_ticket_id:Some("ticket_1".into()),
                before_effect_key:Some("github:issue:42".into()),
            }));
        for invalid in [
            "--list-unresolved-effects --before",
            "--list-unresolved-effects --before 0 ticket_1 github:issue:42",
            "--list-unresolved-effects --before 1000 ../ticket github:issue:42",
            "--list-unresolved-effects --before 1000 ticket_1 ../effect",
            "--list-unresolved-effects --before 1000 ticket_1",
            "--list-unresolved-effects --stop-ticket ticket_1",
            "--list-unresolved-effects --before 1000 ticket_1 github:issue:42 --before 999 ticket_1 github:issue:41",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--list-team-unresolved-effects team_123"),
            Ok(Command::ListTeamUnresolvedEffects {
                orchestration_id:"team_123".into(),
                before_updated_at:None,before_ticket_id:None,before_effect_key:None,
            }));
        assert_eq!(parsed("--list-team-unresolved-effects team_123 --before 1000 ticket_1 github:issue:42"),
            Ok(Command::ListTeamUnresolvedEffects {
                orchestration_id:"team_123".into(),
                before_updated_at:Some(1000),before_ticket_id:Some("ticket_1".into()),
                before_effect_key:Some("github:issue:42".into()),
            }));
        for invalid in [
            "--list-team-unresolved-effects",
            "--list-team-unresolved-effects ../unsafe",
            "--list-team-unresolved-effects team_123 --before",
            "--list-team-unresolved-effects team_123 --before 0 ticket_1 github:issue:42",
            "--list-team-unresolved-effects team_123 --before 1000 ../ticket github:issue:42",
            "--list-team-unresolved-effects team_123 --before 1000 ticket_1 ../effect",
            "--list-team-unresolved-effects team_123 --before 1000 ticket_1",
            "--list-team-unresolved-effects team_123 --stop-ticket ticket_1",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--set-source-mode source_123 2 observe"),
            Ok(Command::SetSourceMode {
                source_id: "source_123".into(), expected_revision: 2, mode: "observe".into(),
            }));
        assert_eq!(parsed("--set-source-mode source_123 4 off"),
            Ok(Command::SetSourceMode {
                source_id: "source_123".into(), expected_revision: 4, mode: "off".into(),
            }));
        for invalid in [
            "--set-source-mode", "--set-source-mode source_123",
            "--set-source-mode source_123 1", "--set-source-mode ../private 1 off",
            "--set-source-mode source_123 0 off", "--set-source-mode source_123 -1 observe",
            "--set-source-mode source_123 nope off", "--set-source-mode source_123 1 auto",
            "--set-source-mode source_123 1 full", "--set-source-mode source_123 1 Observe",
            "--set-source-mode source_123 1 off --observe-source source_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--inspect-source-health source_123"),
            Ok(Command::InspectSourceHealth { source_id: "source_123".into() }));
        for invalid in ["--inspect-source-health", "--inspect-source-health ../private",
                        "--inspect-source-health source_123 --observe-source source_123"] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--inspect-source-decisions source_123"),
            Ok(Command::InspectSourceDecisions {
                source_id: "source_123".into(), paginated: false,
                before_sequence: None, before_delivery_id: None,
            }));
        assert_eq!(parsed("--inspect-source-decisions source_123 --page"),
            Ok(Command::InspectSourceDecisions {
                source_id: "source_123".into(), paginated: true,
                before_sequence: None, before_delivery_id: None,
            }));
        assert_eq!(parsed("--inspect-source-decisions source_123 --before 200 receipt_123"),
            Ok(Command::InspectSourceDecisions {
                source_id: "source_123".into(), paginated: true,
                before_sequence: Some(200), before_delivery_id: Some("receipt_123".into()),
            }));
        for invalid in [
            "--inspect-source-decisions",
            "--inspect-source-decisions ../private",
            "--inspect-source-decisions source_123 --observe-source source_123",
            "--inspect-source-decisions source_123 --before",
            "--inspect-source-decisions source_123 --before 0 receipt_123",
            "--inspect-source-decisions source_123 --before 10",
            "--inspect-source-decisions source_123 --before 10 ../private",
            "--inspect-source-decisions source_123 --page --before 10 receipt_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert!(parsed("--list-review-inbox --stop-ticket ticket_1").is_err());
        assert_eq!(parsed("--list-demands all"),
            Ok(Command::ListDemands {
                state:"all".into(),before_updated_at:None,before_id:None,search_query:None,project_id:None,
            }));
        assert_eq!(parsed("--list-demands blocked --before 2000 ticket_099 --search needle"),
            Ok(Command::ListDemands {
                state:"blocked".into(),before_updated_at:Some(2000),
                before_id:Some("ticket_099".into()),search_query:Some("needle".into()),project_id:None,
            }));
        for invalid in [
            "--list-demands", "--list-demands ALL", "--list-demands ../unsafe",
            "--list-demands blocked --search", "--list-demands blocked --search --stop-ticket",
            "--list-demands blocked --before", "--list-demands blocked --before 0 ticket_099",
            "--list-demands blocked --before 2000 ../escape",
            "--list-demands blocked --search needle --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }

        assert_eq!(parsed("--list-demands blocked --project archived-project --search needle"),
            Ok(Command::ListDemands {
                state:"blocked".into(), before_updated_at:None, before_id:None,
                search_query:Some("needle".into()), project_id:Some("archived-project".into()),
            }));
        assert_eq!(parsed("--list-demands blocked --before 2000 ticket_099 --search needle --project archived-project"),
            Ok(Command::ListDemands {
                state:"blocked".into(), before_updated_at:Some(2000),
                before_id:Some("ticket_099".into()), search_query:Some("needle".into()),
                project_id:Some("archived-project".into()),
            }));
        for invalid in [
            "--list-demands blocked --project",
            "--list-demands blocked --project ../escape",
            "--list-demands blocked --project current.project",
            "--list-demands blocked --project --search private",
            "--list-demands blocked --project current --project current",
            "--list-demands blocked --project current --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--list-team-demands team_123 blocked"),
            Ok(Command::ListTeamDemands {
                orchestration_id:"team_123".into(),state:"blocked".into(),
                before_updated_at:None,before_id:None,search_query:None,project_id:None,
            }));
        assert_eq!(parsed("--list-team-demands team_123 all"),
            Ok(Command::ListTeamDemands {
                orchestration_id:"team_123".into(),state:"all".into(),
                before_updated_at:None,before_id:None,search_query:None,project_id:None,
            }));
        assert_eq!(parsed("--list-team-demands team_123 blocked --before 2000 ticket_099"),
            Ok(Command::ListTeamDemands {
                orchestration_id:"team_123".into(),state:"blocked".into(),
                before_updated_at:Some(2000),before_id:Some("ticket_099".into()),
                search_query:None,project_id:None,
            }));
        assert_eq!(parsed("--list-team-demands team_123 blocked --search cursor"),
            Ok(Command::ListTeamDemands {
                orchestration_id:"team_123".into(),state:"blocked".into(),
                before_updated_at:None,before_id:None,search_query:Some("cursor".into()),project_id:None,
            }));
        assert_eq!(parsed("--list-team-demands team_123 blocked --before 2000 ticket_099 --search cursor"),
            Ok(Command::ListTeamDemands {
                orchestration_id:"team_123".into(),state:"blocked".into(),
                before_updated_at:Some(2000),before_id:Some("ticket_099".into()),
                search_query:Some("cursor".into()),project_id:None,
            }));
        for invalid in [
            "--list-team-demands team_123 blocked --search",
            "--list-team-demands team_123 blocked --search --stop-ticket",
            "--list-team-demands team_123 blocked --search cursor --stop-ticket ticket_123",
            "--list-team-demands team_123 blocked --before 2000 ticket_099 --search",
            "--list-team-demands team_123 blocked --before",
            "--list-team-demands team_123 blocked --before 2",
            "--list-team-demands team_123 blocked --before 0 ticket_099",
            "--list-team-demands team_123 blocked --before -1 ticket_099",
            "--list-team-demands team_123 blocked --before nope ticket_099",
            "--list-team-demands team_123 blocked --before 2000 ../unsafe",
            "--list-team-demands team_123 blocked --before 2000 ticket_099 --stop-ticket t",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }

        assert_eq!(parsed("--list-team-demands team_123 blocked --before 2000 ticket_099 --project archived-project --search needle"),
            Ok(Command::ListTeamDemands {
                orchestration_id:"team_123".into(), state:"blocked".into(),
                before_updated_at:Some(2000), before_id:Some("ticket_099".into()),
                search_query:Some("needle".into()), project_id:Some("archived-project".into()),
            }));
        for invalid in [
            "--list-team-demands team_123 all --project",
            "--list-team-demands team_123 all --project ../escape",
            "--list-team-demands team_123 all --project current.project",
            "--list-team-demands team_123 all --project archived --project archived",
            "--list-team-demands team_123 all --project archived --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert!(parsed("--list-team-demands").is_err());
        assert!(parsed("--list-team-demands team_123").is_err());
        assert!(parsed("--list-team-demands team_123 BLOCKED").is_err());
        assert!(parsed("--list-team-demands ../unsafe running").is_err());
        assert!(parsed("--list-team-demands team_123 running --stop-ticket task").is_err());
        assert_eq!(parsed("--set-team-roster team_123 2 /private/roster.json"),
            Ok(Command::SetTeamRoster {
                orchestration_id:"team_123".into(), expected_revision:2,
                roster_file:PathBuf::from("/private/roster.json"),
            }));
        assert_eq!(parsed("--set-team-attempt-limit team_123 2 3"),
            Ok(Command::SetTeamAttemptLimit {
                orchestration_id:"team_123".into(), expected_revision:2, limit:3,
            }));
        for invalid in [
            "--set-team-roster", "--set-team-roster ../bad 2 roster.json",
            "--set-team-roster team_123 0 roster.json",
            "--set-team-roster team_123 2 --stop-ticket",
            "--set-team-roster team_123 2 roster.json --stop-ticket ticket",
            "--set-team-attempt-limit", "--set-team-attempt-limit team_123 2",
            "--set-team-attempt-limit team_123 2 0",
            "--set-team-attempt-limit team_123 2 5",
            "--set-team-attempt-limit team_123 2 nope",
            "--set-team-attempt-limit team_123 2 2 --stop-ticket ticket",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        let roster_file = std::env::temp_dir().join(format!(
            "sa-headless-roster-{}", crate::storage::id(),
        ));
        std::fs::write(&roster_file,
            r#"{"workerBotIds":["worker"],"reviewerBotIds":[]}"#).unwrap();
        assert_eq!(read_bounded_roster(&roster_file).unwrap().worker_bot_ids,
            vec!["worker".to_string()]);
        std::fs::write(&roster_file,
            r#"{"workerBotIds":["worker"],"reviewerBotIds":[],"unexpected":true}"#).unwrap();
        assert!(read_bounded_roster(&roster_file).is_err());
        std::fs::write(&roster_file, vec![b'x'; 16 * 1024 + 1]).unwrap();
        assert!(read_bounded_roster(&roster_file).is_err());
        std::fs::remove_file(&roster_file).unwrap();
        assert_eq!(parsed("--inspect-team-roster team_123"),
            Ok(Command::InspectTeamRoster{orchestration_id:"team_123".into()}));
        for invalid in [
            "--inspect-team-roster",
            "--inspect-team-roster ../unsafe",
            "--inspect-team-roster team_123 --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(),"{invalid}"); }
        assert_eq!(parsed("--inspect-team-readiness team_123"),
            Ok(Command::InspectTeamReadiness { orchestration_id: "team_123".into() }));
        assert!(parsed("--inspect-team-readiness").is_err());
        assert!(parsed("--inspect-team-readiness ../other").is_err());
        assert!(parsed("--inspect-team-readiness team_123 --stop-ticket ticket_123").is_err());
        assert_eq!(parsed("--create-source team_123 owner/repo dependabot[bot] issues.opened"),
            Ok(Command::CreateSource {
                orchestration_id: "team_123".into(),
                repository: "owner/repo".into(),
                actor_filter: "dependabot[bot]".into(),
                event_kind: "issues.opened".into(),
            }));
        for invalid in [
            "--create-source", "--create-source ../unsafe owner/repo actor issues.opened",
            "--create-source team_123", "--create-source team_123 owner/repo actor",
            "--create-source team_123 owner/repo actor --stop-ticket",
            "--create-source team_123 owner/repo actor issues.opened --stop-ticket task",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--list-team-sources team_123"),
            Ok(Command::ListTeamSources { orchestration_id: "team_123".into() }));
        for invalid in [
            "--list-team-sources", "--list-team-sources ../unsafe",
            "--list-team-sources team_123 --stop-ticket task",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--observe-source source_123"),
            Ok(Command::ObserveSource { source_id: "source_123".into() }));
        assert!(parsed("--observe-source").is_err());
        assert!(parsed("--observe-source ../untrusted").is_err());
        assert!(parsed("--observe-source source_123 --stop-ticket ticket_123").is_err());
        assert_eq!(parsed("--observe-team-sources team_123"),
            Ok(Command::ObserveTeamSources { orchestration_id: "team_123".into() }));
        for invalid in [
            "--observe-team-sources",
            "--observe-team-sources ../unsafe",
            "--observe-team-sources team_123 --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--observe-linked-pr-checks source_123 ticket_123 pull:42"),
            Ok(Command::ObserveLinkedPrChecks {
                source_id:"source_123".into(), ticket_id:"ticket_123".into(),
                pull_key:"pull:42".into(),
            }));
        for invalid in [
            "--observe-linked-pr-checks",
            "--observe-linked-pr-checks source_123",
            "--observe-linked-pr-checks source_123 ticket_123",
            "--observe-linked-pr-checks ../foreign ticket_123 pull:42",
            "--observe-linked-pr-checks source_123 ../foreign pull:42",
            "--observe-linked-pr-checks source_123 ticket_123 issue:42",
            "--observe-linked-pr-checks source_123 ticket_123 pull:042",
            "--observe-linked-pr-checks source_123 ticket_123 pull:42 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--observe-check-run source_123 456"),
            Ok(Command::ObserveCheckRun { source_id:"source_123".into(), check_run_id:456 }));
        for invalid in [
            "--observe-check-run",
            "--observe-check-run source_123",
            "--observe-check-run ../outside 456",
            "--observe-check-run source_123 0",
            "--observe-check-run source_123 -1",
            "--observe-check-run source_123 nope",
            "--observe-check-run source_123 9223372036854775808",
            "--observe-check-run source_123 456 --stop-ticket task",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(
            parsed("--preflight-demand team_123 direct /tmp/objective.txt"),
            Ok(Command::PreflightDemand {
                orchestration_id:"team_123".into(), coordinated:false,
                objective_file:PathBuf::from("/tmp/objective.txt"),
            })
        );
        assert_eq!(
            parsed("--preflight-demand team_123 coordinated /tmp/objective.txt"),
            Ok(Command::PreflightDemand {
                orchestration_id:"team_123".into(), coordinated:true,
                objective_file:PathBuf::from("/tmp/objective.txt"),
            })
        );
        for invalid in [
            "--preflight-demand", "--preflight-demand team_123",
            "--preflight-demand team_123 direct",
            "--preflight-demand ../other direct /tmp/objective.txt",
            "--preflight-demand team_123 auto /tmp/objective.txt",
            "--preflight-demand team_123 direct --stop-ticket",
            "--preflight-demand team_123 direct /tmp/objective.txt --stop-ticket ticket_1",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(
            parsed("--preflight-worker-demand team_123 second /tmp/objective.txt"),
            Ok(Command::PreflightWorkerDemand {
                orchestration_id: "team_123".into(),
                worker_bot_id: "second".into(),
                objective_file: PathBuf::from("/tmp/objective.txt"),
            })
        );
        for invalid in [
            "--preflight-worker-demand",
            "--preflight-worker-demand team_123",
            "--preflight-worker-demand team_123 second",
            "--preflight-worker-demand ../other second /tmp/objective.txt",
            "--preflight-worker-demand team_123 ../second /tmp/objective.txt",
            "--preflight-worker-demand team_123 second --stop-ticket",
            "--preflight-worker-demand team_123 second /tmp/objective.txt --stop-ticket ticket",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        let filename = std::env::temp_dir().join(format!(
            "sa-preflight-objective-{}", crate::storage::id(),
        ));
        std::fs::write(&filename, "Scope-only objective").unwrap();
        assert_eq!(read_bounded_objective(&filename).unwrap(), "Scope-only objective");
        std::fs::write(&filename, vec![b'x'; 16 * 1024 + 1]).unwrap();
        assert!(read_bounded_objective(&filename).is_err());
        std::fs::write(&filename, [0xff, 0xfe]).unwrap();
        assert!(read_bounded_objective(&filename).is_err());
        std::fs::remove_file(&filename).unwrap();
        assert_eq!(parsed("--queue-demand team_123 request.1 /tmp/objective.txt"),
            Ok(Command::QueueDemand {
                orchestration_id:"team_123".into(), request_key:"request.1".into(),
                objective_file:PathBuf::from("/tmp/objective.txt"),
            }));
        for invalid in [
            "--queue-demand", "--queue-demand team_123",
            "--queue-demand team_123 key",
            "--queue-demand ../other key /tmp/objective.txt",
            "--queue-demand team_123 ../key /tmp/objective.txt",
            "--queue-demand team_123 key --stop-ticket",
            "--queue-demand team_123 key /tmp/objective.txt --stop-ticket ticket",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--queue-worker-demand team_123 secondary-worker request.2 /tmp/objective.txt"),
            Ok(Command::QueueSelectedWorkerDemand {
                orchestration_id: "team_123".into(),
                worker_bot_id: "secondary-worker".into(),
                request_key: "request.2".into(),
                objective_file: PathBuf::from("/tmp/objective.txt"),
            }));
        for invalid in [
            "--queue-worker-demand",
            "--queue-worker-demand team_123",
            "--queue-worker-demand team_123 worker",
            "--queue-worker-demand team_123 worker key",
            "--queue-worker-demand ../unsafe worker key /tmp/objective.txt",
            "--queue-worker-demand team_123 ../worker key /tmp/objective.txt",
            "--queue-worker-demand team_123 worker ../key /tmp/objective.txt",
            "--queue-worker-demand team_123 worker key --stop-ticket",
            "--queue-worker-demand team_123 worker key /tmp/objective.txt --stop-ticket ticket",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--inspect-ticket-source ticket_123"),
            Ok(Command::InspectTicketSource {
                ticket_id:"ticket_123".into(),paginated:false,
                before_at:None,before_source_id:None,before_delivery_id:None,
            }));
        assert_eq!(parsed("--inspect-ticket-source ticket_123 --page"),
            Ok(Command::InspectTicketSource {
                ticket_id:"ticket_123".into(),paginated:true,
                before_at:None,before_source_id:None,before_delivery_id:None,
            }));
        assert_eq!(parsed("--inspect-ticket-source ticket_123 --before 5000 source_1 receipt_1"),
            Ok(Command::InspectTicketSource {
                ticket_id:"ticket_123".into(),paginated:true,
                before_at:Some(5000),before_source_id:Some("source_1".into()),
                before_delivery_id:Some("receipt_1".into()),
            }));
        for invalid in [
            "--inspect-ticket-source",
            "--inspect-ticket-source ../unsafe",
            "--inspect-ticket-source ticket_123 --stop-ticket ticket_123",
            "--inspect-ticket-source ticket_123 --before",
            "--inspect-ticket-source ticket_123 --before 0 source_1 receipt_1",
            "--inspect-ticket-source ticket_123 --before 5000 ../source receipt_1",
            "--inspect-ticket-source ticket_123 --before 5000 source_1",
            "--inspect-ticket-source ticket_123 --page --before 5000 source_1 receipt_1",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--list-ticket-source-links ticket_123"),
            Ok(Command::ListTicketSourceLinks { ticket_id: "ticket_123".into() }));
        for invalid in [
            "--list-ticket-source-links",
            "--list-ticket-source-links ../unsafe",
            "--list-ticket-source-links ticket_123 --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert_eq!(parsed("--link-source-thread source_123 ticket_123 issue:42"),
            Ok(Command::LinkSourceThread {
                source_id:"source_123".into(),ticket_id:"ticket_123".into(),
                subject_key:"issue:42".into(),
            }));
        assert_eq!(parsed("--link-source-thread source_123 ticket_123 pull:99"),
            Ok(Command::LinkSourceThread {
                source_id:"source_123".into(),ticket_id:"ticket_123".into(),
                subject_key:"pull:99".into(),
            }));
        for invalid in [
            "--link-source-thread",
            "--link-source-thread source_123",
            "--link-source-thread source_123 ticket_123",
            "--link-source-thread ../unsafe ticket_123 issue:42",
            "--link-source-thread source_123 ../unsafe pull:99",
            "--link-source-thread source_123 ticket_123 issue:01",
            "--link-source-thread source_123 ticket_123 pull:abc",
            "--link-source-thread source_123 ticket_123 issue:42 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--link-issue-pull source_123 ticket_123 issue:42 pull:99"),
            Ok(Command::LinkIssuePull {
                source_id:"source_123".into(), ticket_id:"ticket_123".into(),
                issue_key:"issue:42".into(), pull_key:"pull:99".into(),
            }));
        assert_eq!(parsed("--verify-link-issue-pull source_123 ticket_123 issue:42 pull:99"),
            Ok(Command::VerifyLinkIssuePull {
                source_id:"source_123".into(), ticket_id:"ticket_123".into(),
                issue_key:"issue:42".into(), pull_key:"pull:99".into(),
            }));
        for invalid in [
            "--verify-link-issue-pull",
            "--verify-link-issue-pull source_123",
            "--verify-link-issue-pull source_123 ticket_123 issue:42",
            "--verify-link-issue-pull ../unsafe ticket_123 issue:42 pull:99",
            "--verify-link-issue-pull source_123 ticket_123 pull:42 issue:99",
            "--verify-link-issue-pull source_123 ticket_123 issue:42 pull:01",
            "--verify-link-issue-pull source_123 ticket_123 issue:42 pull:99 --stop-ticket ticket_123",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        for invalid in [
            "--link-issue-pull",
            "--link-issue-pull source_123",
            "--link-issue-pull source_123 ticket_123",
            "--link-issue-pull source_123 ticket_123 issue:42",
            "--link-issue-pull ../bad ticket_123 issue:42 pull:99",
            "--link-issue-pull source_123 ../bad issue:42 pull:99",
            "--link-issue-pull source_123 ticket_123 pull:42 issue:99",
            "--link-issue-pull source_123 ticket_123 issue:0 pull:99",
            "--link-issue-pull source_123 ticket_123 issue:42 pull:00",
            "--link-issue-pull source_123 ticket_123 issue:42 pull:99 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--unlink-source-thread source_123 ticket_123 issue:42"),
            Ok(Command::UnlinkSourceThread {
                source_id:"source_123".into(),ticket_id:"ticket_123".into(),
                subject_key:"issue:42".into(),
            }));
        assert_eq!(parsed("--unlink-source-thread source_123 ticket_123 pull:99"),
            Ok(Command::UnlinkSourceThread {
                source_id:"source_123".into(),ticket_id:"ticket_123".into(),
                subject_key:"pull:99".into(),
            }));
        for invalid in [
            "--unlink-source-thread",
            "--unlink-source-thread source_123",
            "--unlink-source-thread source_123 ticket_123",
            "--unlink-source-thread ../unsafe ticket_123 issue:42",
            "--unlink-source-thread source_123 ../unsafe issue:42",
            "--unlink-source-thread source_123 ticket_123 issue:0",
            "--unlink-source-thread source_123 ticket_123 issue:01",
            "--unlink-source-thread source_123 ticket_123 pull:abc",
            "--unlink-source-thread source_123 ticket_123 issue:42 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--inspect-ticket-effects ticket_123"),
            Ok(Command::InspectTicketEffects {
                ticket_id: "ticket_123".into(), before_created_at: None,
                before_key: None, unresolved_only: false,
            }));
        assert_eq!(parsed("--inspect-ticket-effects ticket_123 --before 1000 github:issue:42 --unresolved-only"),
            Ok(Command::InspectTicketEffects {
                ticket_id: "ticket_123".into(), before_created_at: Some(1000),
                before_key: Some("github:issue:42".into()), unresolved_only: true,
            }));
        assert_eq!(parsed("--inspect-ticket-effects ticket_123 --unresolved-only --before 1000 github:issue:42"),
            Ok(Command::InspectTicketEffects {
                ticket_id: "ticket_123".into(), before_created_at: Some(1000),
                before_key: Some("github:issue:42".into()), unresolved_only: true,
            }));
        assert!(parsed("--inspect-ticket-effects").is_err());
        assert!(parsed("--inspect-ticket-effects ../unsafe").is_err());
        for invalid in [
            "--inspect-ticket-effects ticket_123 --before",
            "--inspect-ticket-effects ticket_123 --before 1000",
            "--inspect-ticket-effects ticket_123 --before 0 github:issue:42",
            "--inspect-ticket-effects ticket_123 --before nope github:issue:42",
            "--inspect-ticket-effects ticket_123 --before 1000 ../unsafe",
            "--inspect-ticket-effects ticket_123 --unresolved-only --unresolved-only",
            "--inspect-ticket-effects ticket_123 --before 1000 github:issue:42 --before 999 github:issue:41",
            "--inspect-ticket-effects ticket_123 --unresolved-only --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert!(parsed("--inspect-ticket-effects ticket_123 --stop-ticket ticket_123").is_err());
        assert_eq!(parsed("--inspect-ticket-graph ticket_123"),
            Ok(Command::InspectTicketGraph { ticket_id: "ticket_123".into() }));
        for invalid in [
            "--inspect-ticket-graph",
            "--inspect-ticket-graph ../outside",
            "--inspect-ticket-graph ticket_123 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--inspect-ticket ticket_123"),
            Ok(Command::InspectTicket { ticket_id: "ticket_123".into() }));
        assert_eq!(parsed("--accept-ticket ticket_123 4"), Ok(Command::AcceptTicket {
            ticket_id: "ticket_123".into(), expected_revision: 4,
        }));
        assert_eq!(parse(&["--reject-ticket".into(), "ticket_123".into(), "7".into(), "Needs changes".into()]),
            Ok((Command::RejectTicket {
                ticket_id: "ticket_123".into(), expected_revision: 7, reason: "Needs changes".into(),
            }, None)));
        assert_eq!(parsed("--quarantine-detached-ticket ticket_123 8"),
            Ok(Command::QuarantineDetachedTicket {
                ticket_id:"ticket_123".into(),expected_revision:8,
            }));
        for invalid in [
            "--quarantine-detached-ticket",
            "--quarantine-detached-ticket ticket_123",
            "--quarantine-detached-ticket ../unsafe 8",
            "--quarantine-detached-ticket ticket_123 nope",
            "--quarantine-detached-ticket ticket_123 -1",
            "--quarantine-detached-ticket ticket_123 8 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(),"{invalid}");
        }
        assert_eq!(parsed("--quarantine-expired-ticket ticket_123 8"),
            Ok(Command::QuarantineExpiredTicket {
                ticket_id:"ticket_123".into(),expected_revision:8,
            }));
        for invalid in [
            "--quarantine-expired-ticket",
            "--quarantine-expired-ticket ticket_123",
            "--quarantine-expired-ticket ../unsafe 8",
            "--quarantine-expired-ticket ticket_123 nope",
            "--quarantine-expired-ticket ticket_123 -1",
            "--quarantine-expired-ticket ticket_123 8 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(),"{invalid}");
        }
        let digest = "a".repeat(64);
        let valid = format!("--quarantine-pending-effect ticket_123 8 github:comment:1 {digest}");
        assert_eq!(parsed(&valid),
            Ok(Command::QuarantinePendingEffect {
                ticket_id: "ticket_123".into(),
                expected_revision: 8,
                effect_key: "github:comment:1".into(),
                payload_sha256: digest.clone(),
            }));
        for invalid in [
            "--quarantine-pending-effect",
            "--quarantine-pending-effect ticket_123 8",
            "--quarantine-pending-effect ../unsafe 8 github:comment:1",
            "--quarantine-pending-effect ticket_123 -1 github:comment:1",
            "--quarantine-pending-effect ticket_123 8 ../effect",
            "--quarantine-pending-effect ticket_123 8 github:comment:1 bad",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert!(parsed(&format!(
            "--quarantine-pending-effect ticket_123 8 github:comment:1 {}",
            "A".repeat(64),
        )).is_err(), "noncanonical SHA-256 must be rejected");
        assert!(parsed(&format!("{valid} --stop-ticket ticket_123")).is_err());

        assert_eq!(parsed("--quarantine-orphan-ticket ticket_123 8"),
            Ok(Command::QuarantineOrphanRunning {
                ticket_id:"ticket_123".into(),expected_revision:8,
            }));
        for invalid in [
            "--quarantine-orphan-ticket",
            "--quarantine-orphan-ticket ticket_123",
            "--quarantine-orphan-ticket ../unsafe 8",
            "--quarantine-orphan-ticket ticket_123 nope",
            "--quarantine-orphan-ticket ticket_123 -1",
            "--quarantine-orphan-ticket ticket_123 8 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(),"{invalid}");
        }
        assert_eq!(parsed("--quarantine-terminal-orphan ticket_123 8"),
            Ok(Command::QuarantineTerminalOrphan {
                ticket_id: "ticket_123".into(), expected_revision: 8,
            }));
        for invalid in [
            "--quarantine-terminal-orphan",
            "--quarantine-terminal-orphan ticket_123",
            "--quarantine-terminal-orphan ../unsafe 8",
            "--quarantine-terminal-orphan ticket_123 nope",
            "--quarantine-terminal-orphan ticket_123 -1",
            "--quarantine-terminal-orphan ticket_123 8 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--stop-ticket ticket_123"), Ok(Command::StopTicket {
            ticket_id: "ticket_123".into(),
        }));
        assert_eq!(parsed("--stop-ticket-at-revision ticket_123 4"),
            Ok(Command::StopTicketAtRevision {
                ticket_id: "ticket_123".into(), expected_revision: 4,
            }));
        for invalid in [
            "--stop-ticket-at-revision",
            "--stop-ticket-at-revision ticket_123",
            "--stop-ticket-at-revision ../unsafe 4",
            "--stop-ticket-at-revision ticket_123 nope",
            "--stop-ticket-at-revision ticket_123 -1",
            "--stop-ticket-at-revision ticket_123 4 --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert!(parsed("--accept-ticket ticket_123").is_err());
        assert!(parsed("--accept-ticket ticket_123 nope").is_err());
        assert!(parsed("--accept-ticket ../outside 1").is_err());
        assert!(parsed("--reject-ticket ticket_123 1").is_err());
        assert!(parsed("--reject-ticket ticket_123 1 --sign-out").is_err());
        assert!(parsed("--stop-ticket").is_err());
        assert!(parsed("--stop-ticket ticket_123 --list-tickets").is_err());
        assert_eq!(parsed("--disable-orchestration team_123 3"), Ok(Command::DisableOrchestration {
            orchestration_id: "team_123".into(), expected_revision: 3,
        }));
        assert_eq!(parsed("--enable-orchestration team_123 3"), Ok(Command::EnableOrchestration {
            orchestration_id: "team_123".into(), expected_revision: 3,
        }));
        for invalid in [
            "--enable-orchestration",
            "--enable-orchestration team_123",
            "--enable-orchestration team_123 0",
            "--enable-orchestration team_123 -1",
            "--enable-orchestration team_123 nope",
            "--enable-orchestration ../unsafe 3",
            "--enable-orchestration team_123 3 --list-tickets",
        ] { assert!(parsed(invalid).is_err(), "{invalid}"); }
        assert!(parsed("--disable-orchestration team_123").is_err());
        assert!(parsed("--disable-orchestration team_123 nope").is_err());
        assert!(parsed("--disable-orchestration ../team 3").is_err());
        assert!(parsed("--disable-orchestration team_123 3 --stop-ticket ticket_123").is_err());
        assert_eq!(parsed("--rename-orchestration team_123 3 Renamed"),
            Ok(Command::RenameOrchestration {
                orchestration_id: "team_123".into(), expected_revision: 3,
                name: "Renamed".into(),
            }));
        for invalid in [
            "--rename-orchestration",
            "--rename-orchestration team_123",
            "--rename-orchestration team_123 3",
            "--rename-orchestration team_123 nope Renamed",
            "--rename-orchestration ../unsafe 3 Renamed",
            "--rename-orchestration team_123 3 --stop-ticket",
            "--rename-orchestration team_123 3 Renamed --stop-ticket ticket_123",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--set-team-strategy team_123 3 economy purpose.txt"),
            Ok(Command::SetTeamStrategy {
                orchestration_id: "team_123".into(), expected_revision: 3,
                strategy: "economy".into(), purpose_file: PathBuf::from("purpose.txt"),
            }));
        for invalid in [
            "--set-team-strategy", "--set-team-strategy team_123",
            "--set-team-strategy team_123 3", "--set-team-strategy team_123 3 economy",
            "--set-team-strategy team_123 nope economy purpose.txt",
            "--set-team-strategy ../unsafe 3 economy purpose.txt",
            "--set-team-strategy team_123 3 unrestricted purpose.txt",
            "--set-team-strategy team_123 3 economy --sign-out",
            "--set-team-strategy team_123 3 economy purpose.txt --list-tickets",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert!(parsed("--accept-ticket ticket_123 1 --import-bots bots.json").is_err());
        assert!(parsed("--inspect-ticket").is_err());
        assert!(parsed("--inspect-ticket ../outside").is_err());
        assert!(parsed("--inspect-ticket x --inspect-orchestrations").is_err());
        assert!(parsed("--list-tickets --inspect-ticket x").is_err());
        assert!(parsed("--wat").is_err());
        assert!(parsed("--import-orchestrations").is_err());
        assert!(parsed("--export-orchestrations").is_err());
        assert!(parsed("--import-orchestrations a.json --export-bots b.json").is_err());
        assert!(parsed("--import-orchestrations a.json --import-bots b.json").is_err());
        assert!(parsed("--device-name").is_err());
        assert!(parsed("--export-bots").is_err());
        assert_eq!(parsed("--sync-bots sources.json"), Ok(Command::SyncBots { path: PathBuf::from("sources.json") }));
        assert!(parsed("--sync-bots").is_err());
        assert!(parsed("--sync-bots sources.json --import-bots bots.json").is_err());
        assert!(parsed("--sync-bots sources.json --sign-in").is_err());
    }

    #[test]
    fn team_mutations_reject_zero_and_negative_policy_revisions_without_rejecting_new_tickets() {
        // A new Team has policy revision 1, while new Tickets have revision 0.
        for invalid in [
            "--disable-orchestration team_123 0",
            "--enable-orchestration team_123 0",
            "--rename-orchestration team_123 0 Renamed",
            "--duplicate-orchestration team_123 0 Copy",
            "--set-team-roster team_123 0 roster.json",
            "--set-team-attempt-limit team_123 0 2",
            "--set-team-strategy team_123 0 balanced purpose.txt",
            "--disable-orchestration team_123 -1",
            "--enable-orchestration team_123 -1",
            "--rename-orchestration team_123 -1 Renamed",
            "--duplicate-orchestration team_123 -1 Copy",
            "--set-team-roster team_123 -1 roster.json",
            "--set-team-attempt-limit team_123 -1 2",
            "--set-team-strategy team_123 -1 balanced purpose.txt",
        ] {
            assert!(parsed(invalid).is_err(), "{invalid}");
        }
        assert_eq!(parsed("--accept-ticket ticket_123 0"),
            Ok(Command::AcceptTicket {
                ticket_id: "ticket_123".into(), expected_revision: 0,
            }));
    }

    #[test]
    fn import_bots_is_a_modifier_not_an_op() {
        let (_, import) = parse(&argv("--import-bots bots.json")).unwrap();
        assert_eq!(import, Some(PathBuf::from("bots.json")));
        let (_, import) = parse(&argv("--sign-in --import-bots bots.json --device-name s")).unwrap();
        assert_eq!(import, Some(PathBuf::from("bots.json")));
        assert_eq!(parse(&argv("--import-bots bots.json")).unwrap().0, Command::Serve);
        // It still conflicts with the exclusive ops, which exit after saving.
        assert!(parse(&argv("--sign-out --import-bots bots.json")).is_err());
    }

    #[test]
    fn help_mentions_every_flag() {
        let text = help();
        for flag in [
            "--sign-in",
            "--sign-out",
            "--full-access",
            "--no-full-access",
            "--device-name",
            "--import-bots",
            "--export-bots",
            "--preview-orchestrations",
            "--import-orchestrations",
            "--export-orchestrations",
            "--inspect-orchestrations",
            "--list-tickets",
            "--list-ticket-activity",
            "--list-team-member-activity",
            "--list-review-inbox",
            "--list-team-review-inbox",
            "--list-recovery-inbox",
            "--list-team-unresolved-effects",
            "--list-demands",
            "--inspect-team-readiness",
            "--inspect-team-roster",
            "--inspect-ticket",
            "--inspect-ticket-graph",
            "--inspect-ticket-effects",
            "--inspect-ticket-source",
            "--list-ticket-source-links",
            "--queue-worker-demand",
            "--inspect-source-decisions",
            "--link-source-thread",
            "--unlink-source-thread",
            "--observe-source",
            "--observe-team-sources",
            "--observe-check-run",
            "--observe-linked-pr-checks",
            "--accept-ticket",
            "--reject-ticket",
            "--stop-ticket",
            "--stop-ticket-at-revision",
            "--quarantine-detached-ticket",
            "--disable-orchestration",
            "--enable-orchestration",
            "--rename-orchestration",
            "--duplicate-orchestration",
            "--set-team-strategy",
            "--sync-bots",
            "xvfb-run",
        ] {
            assert!(text.contains(flag), "{flag}");
        }
    }

    #[test]
    fn only_live_unlimited_tasks_take_their_queue() {
        assert!(can_drain(false, None, None, 1000));
        assert!(can_drain(false, None, Some(1000), 1000));
        assert!(can_drain(false, Some(500), Some(1000), 1000));
        // Inside a limit (with or without a known reset), or archived: hold. limitActive
        // holds a limit with an unknown reset indefinitely; the resume send ends a run,
        // which then delivers the queue.
        assert!(!can_drain(false, Some(500), Some(1001), 1000));
        assert!(!can_drain(false, Some(500), None, 1000));
        assert!(!can_drain(true, None, None, 1000));
        assert!(!can_drain(true, None, Some(999), 1000));
        assert!(!can_drain(true, Some(500), Some(999), 1000));
    }

    #[test]
    fn ops_conflict() {
        assert!(parse(&argv("--sign-in --sign-out")).is_err());
        assert!(parse(&argv("--full-access --no-full-access")).is_err());
        assert!(parse(&argv("--sign-out --full-access")).is_err());
        // A name with a lone op is fine, in either order.
        assert!(parse(&argv("--sign-in --device-name x")).is_ok());
    }
}
