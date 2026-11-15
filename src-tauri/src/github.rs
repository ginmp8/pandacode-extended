//! GitHub through the user's own `gh` CLI. PandaCode stores no tokens: every call
//! runs the signed-in CLI with a null stdin, bounded output, and a deadline, and
//! only ever from a project's checkout so the repository is the remote's.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command, time::Duration};

use crate::workspace::{checked, run_process, Capture};

pub(crate) const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const OBSERVE_OUTPUT_LIMIT: usize = 64 * 1024;
const BODY_LIMIT: usize = 4 * 1024;
const IMPORT_BODY_LIMIT: usize = 16 * 1024;
pub const MAX_REFERENCES: usize = 5;
pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);
const OBSERVE_EVENT_KINDS: &[&str] = &[
    "issues.opened",
    "issues.edited",
    "issue_comment.created",
    "pull_request.opened",
    "pull_request_review.submitted",
    "check_run.completed",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObserveEvent {
    pub repository: String,
    pub actor: String,
    pub event_kind: String,
    pub subject_key: String,
    pub delivery_id: String,
    pub sequence: i64,
}

pub(crate) fn supports_observe_event(event_kind: &str) -> bool {
    OBSERVE_EVENT_KINDS.contains(&event_kind)
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub name_with_owner: String,
    pub default_branch: String,
    pub url: String,
}
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GithubStatus {
    pub installed: bool,
    pub version: String,
    pub authenticated: bool,
    pub login: Option<String>,
    pub repo: Option<Repository>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub kind: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub url: String,
    pub author: String,
}
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub url: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub updated_at: String,
}
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub kind: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub url: String,
    pub author: String,
    pub body: String,
    pub labels: Vec<String>,
    pub truncated: bool,
}
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkState {
    pub kind: String,
    pub number: i64,
    pub state: String,
    pub title: String,
    pub url: String,
}
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: i64,
    pub url: String,
    pub title: String,
    pub state: String,
}
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IssueFilter {
    #[serde(default)]
    pub assigned_to_me: bool,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
}

/// `gh` as the user's shell finds it. Apps opened from Finder or the Dock inherit a
/// minimal PATH without Homebrew, so look where coding CLIs are found too, and pass that
/// PATH on for the `git` that `gh` runs.
fn gh_base(root: &Path) -> Command {
    let program = crate::providers::tool_path("gh").unwrap_or_else(|| "gh".into());
    let mut command = Command::new(program);
    command.current_dir(root);
    if let Ok(path) = crate::providers::executable_path() {
        command.env("PATH", path);
    }
    command
}
fn gh_command(root: &Path, args: &[&str]) -> Command {
    let mut command = gh_base(root);
    command
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_PAGER", "cat")
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0");
    command
}
pub(crate) fn run_gh(root: &Path, args: &[&str], timeout: Duration) -> Result<Capture, String> {
    run_gh_limited(root, args, OUTPUT_LIMIT, timeout)
}
fn run_gh_limited(
    root: &Path,
    args: &[&str],
    limit: usize,
    timeout: Duration,
) -> Result<Capture, String> {
    checked(run_process(gh_command(root, args), "GitHub CLI", limit, timeout)?)
}
pub(crate) fn json(capture: &Capture) -> Result<serde_json::Value, String> {
    if capture.truncated {
        return Err("The GitHub response was too large to read.".into());
    }
    serde_json::from_slice(&capture.bytes).map_err(|_| "GitHub returned an unexpected response.".into())
}
pub(crate) fn text(value: &serde_json::Value, key: &str) -> String {
    value.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
}
pub(crate) fn names(value: &serde_json::Value, key: &str, field: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(|item| item.get(field)?.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}
pub(crate) fn state_of(value: &serde_json::Value) -> String {
    let state = text(value, "state").to_ascii_lowercase();
    if state == "open" && value.get("isDraft").and_then(|v| v.as_bool()) == Some(true) {
        "draft".into()
    } else {
        state
    }
}
/// Cut at a character boundary and say so, so an excerpt never pretends to be whole.
pub(crate) fn excerpt(body: &str, limit: usize) -> (String, bool) {
    if body.len() <= limit {
        return (body.to_string(), false);
    }
    let mut end = limit;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    (body[..end].to_string(), true)
}
fn valid_query(query: &str) -> Result<String, String> {
    let query = query.trim();
    if query.len() > 200 || query.contains('\0') {
        return Err("Shorten the GitHub search.".into());
    }
    Ok(query.to_string())
}
pub(crate) fn flag(name: &str, value: &str) -> String {
    // The `--flag=value` form keeps user text out of the CLI's option parser.
    format!("--{name}={value}")
}

fn observe_subject(subject_key: &str) -> Result<(&str, i64), String> {
    let Some((kind, number)) = subject_key.split_once(':') else {
        return Err("Choose an exact linked issue or pull request.".into());
    };
    if !matches!(kind, "issue" | "pull")
        || number.is_empty()
        || number.len() > 12
        || number.starts_with('0')
        || !number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("Choose an exact linked issue or pull request.".into());
    }
    let number = number.parse::<i64>().map_err(|_| "Invalid linked thread number.")?;
    if number <= 0 {
        return Err("Invalid linked thread number.".into());
    }
    Ok((kind, number))
}

fn observe_repository(repository: &str) -> Result<(&str, &str), String> {
    if repository.len() > 200 || repository != repository.to_ascii_lowercase() {
        return Err("The configured repository is not canonical.".into());
    }
    let Some((owner, name)) = repository.split_once('/') else {
        return Err("The configured repository is incomplete.".into());
    };
    let token = |value: &str| {
        !value.is_empty()
            && value.len() <= 100
            && !value.starts_with('.')
            && !value.contains("..")
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
            })
    };
    if !token(owner) || !token(name) {
        return Err("The configured repository is not a valid owner/name.".into());
    }
    Ok((owner, name))
}

fn observe_timestamp(value: &serde_json::Value, field: &str) -> Result<i64, String> {
    let raw = value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or("GitHub did not return required event timestamp metadata.")?;
    let at = chrono::DateTime::parse_from_rfc3339(raw)
        .map_err(|_| "GitHub returned invalid event timestamp metadata.")?
        .timestamp_millis();
    if at <= 0 {
        return Err("GitHub returned invalid event timestamp metadata.".into());
    }
    Ok(at)
}

fn observe_edit_delivery_id(global_id: &str) -> Result<String, String> {
    if global_id.is_empty() || global_id.len() > 256 || !global_id.is_ascii() {
        return Err("GitHub did not return a stable issue edit ID.".into());
    }
    let digest = Sha256::digest(global_id.as_bytes());
    let hex = b"0123456789abcdef";
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex[(byte >> 4) as usize] as char);
        encoded.push(hex[(byte & 0x0f) as usize] as char);
    }
    Ok(format!("gh-issues-edited-{encoded}"))
}

/// Fetch one source-allowlisted event for one exact, already-linked issue/PR.
/// Queries select only bounded event identity, actor, status and ordering
/// metadata; they never select issue/PR text, comments, diffs or model content.
/// `gh auth status` runs first.
pub(crate) fn observe_event(
    root: &Path,
    repository: &str,
    subject_key: &str,
    event_kind: &str,
) -> Result<ObserveEvent, String> {
    observe_event_with(repository, subject_key, event_kind, |args, limit, timeout| {
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        run_gh_limited(root, &borrowed, limit, timeout)
    })
}

/// Injectable CLI boundary for deterministic native tests. Production calls the
/// user's `gh` with the same bounded arguments and authenticated check.
pub(crate) fn observe_event_with(
    repository: &str,
    subject_key: &str,
    event_kind: &str,
    mut run: impl FnMut(&[String], usize, Duration) -> Result<Capture, String>,
) -> Result<ObserveEvent, String> {
    let (owner, name) = observe_repository(repository)?;
    let (subject_kind, number) = observe_subject(subject_key)?;
    if !supports_observe_event(event_kind) {
        return Err("This event class is not supported by manual Observe intake.".into());
    }
    if matches!(event_kind, "issues.opened" | "issues.edited") && subject_kind != "issue"
        || matches!(event_kind, "pull_request.opened" | "pull_request_review.submitted" | "check_run.completed")
            && subject_kind != "pull"
    {
        return Err("The linked thread kind does not match the configured event class.".into());
    }

    let auth_args = vec![
        "auth".to_string(),
        "status".to_string(),
        flag("hostname", "github.com"),
    ];
    run(&auth_args, OBSERVE_OUTPUT_LIMIT, Duration::from_secs(10))?;

    let fragments = match event_kind {
        "issues.opened" | "pull_request.opened" => {
            "... on Issue { fullDatabaseId number author { login } } \
             ... on PullRequest { fullDatabaseId number author { login } }"
        }
        "issues.edited" => {
            "... on Issue { number createdAt userContentEdits(last: 1) { nodes { id editedAt editor { login } } } }"
        }
        "issue_comment.created" => {
            "... on Issue { number comments(last: 1) { nodes { fullDatabaseId author { login } } } } \
             ... on PullRequest { number comments(last: 1) { nodes { fullDatabaseId author { login } } } }"
        }
        "pull_request_review.submitted" => {
            "... on PullRequest { number reviews(last: 1) { nodes { fullDatabaseId submittedAt author { login } } } }"
        }
        "check_run.completed" => {
            "... on PullRequest { number commits(last: 1) { nodes { commit { checkSuites(last: 1) { nodes { checkRuns(last: 1) { nodes { fullDatabaseId status completedAt checkSuite { app { slug } } } } } } } } }"
        }
        _ => unreachable!("the supported event list is exhaustive"),
    };
    let query = format!(
        "query($owner: String!, $name: String!, $number: Int!) {{ \
           repository(owner: $owner, name: $name) {{ nameWithOwner \
             issueOrPullRequest(number: $number) {{ __typename {fragments} }} \
           }} \
         }}"
    );
    let args = vec![
        "api".to_string(),
        "graphql".to_string(),
        flag("hostname", "github.com"),
        flag("raw-field", &format!("owner={owner}")),
        flag("raw-field", &format!("name={name}")),
        // `gh api --field` type-converts the validated numeric value so the
        // GraphQL Int variable is sent as a number rather than a string.
        flag("field", &format!("number={number}")),
        flag("raw-field", &format!("query={query}")),
    ];
    let capture = run(&args, OBSERVE_OUTPUT_LIMIT, TIMEOUT)?;
    let response = json(&capture)?;
    if let Some(errors) = response.get("errors") {
        if !errors.as_array().is_some_and(Vec::is_empty) {
            return Err("GitHub returned GraphQL errors for the configured thread.".into());
        }
    }
    let repo_node = response
        .pointer("/data/repository")
        .ok_or("GitHub did not return the configured repository metadata.")?;
    let actual_repository = text(repo_node, "nameWithOwner").to_ascii_lowercase();
    if actual_repository != repository {
        return Err("GitHub returned a different repository than the configured source.".into());
    }
    let thread = repo_node
        .get("issueOrPullRequest")
        .filter(|node| !node.is_null())
        .ok_or("GitHub did not return the exact linked thread.")?;
    let actual_kind = match text(thread, "__typename").as_str() {
        "Issue" => "issue",
        "PullRequest" => "pull",
        _ => return Err("GitHub returned an unsupported thread type.".into()),
    };
    let actual_number = thread
        .get("number")
        .and_then(serde_json::Value::as_i64)
        .ok_or("GitHub returned an invalid thread number.")?;
    let actual_subject = format!("{actual_kind}:{actual_number}");
    if actual_subject != subject_key {
        return Err("GitHub returned a different thread than the configured link.".into());
    }

    let (derived_kind, event_node) = match (event_kind, actual_kind) {
        ("issues.opened", "issue") => ("issues.opened", thread),
        ("issues.edited", "issue") => (
            "issues.edited",
            thread
                .pointer("/userContentEdits/nodes/0")
                .filter(|node| !node.is_null())
                .ok_or("GitHub returned no issue edit metadata for this thread.")?,
        ),
        ("pull_request.opened", "pull") => ("pull_request.opened", thread),
        ("issue_comment.created", "issue" | "pull") => (
            "issue_comment.created",
            thread
                .pointer("/comments/nodes/0")
                .filter(|node| !node.is_null())
                .ok_or("GitHub returned no comment metadata for this thread.")?,
        ),
        ("pull_request_review.submitted", "pull") => (
            "pull_request_review.submitted",
            thread
                .pointer("/reviews/nodes/0")
                .filter(|node| !node.is_null())
                .ok_or("GitHub returned no submitted review metadata for this pull request.")?,
        ),
        ("check_run.completed", "pull") => (
            "check_run.completed",
            thread
                .pointer("/commits/nodes/0/commit/checkSuites/nodes/0/checkRuns/nodes/0")
                .filter(|node| !node.is_null())
                .ok_or("GitHub returned no check run metadata for this pull request.")?,
        ),
        _ => return Err("The linked thread does not match the configured event class.".into()),
    };
    if derived_kind == "pull_request_review.submitted"
        && event_node.get("submittedAt").and_then(serde_json::Value::as_str).is_none()
    {
        return Err("GitHub returned a pending review rather than a submitted review.".into());
    }
    let sequence = match derived_kind {
        "issues.edited" => {
            let edited_at = observe_timestamp(event_node, "editedAt")?;
            let created_at = observe_timestamp(thread, "createdAt")?;
            if edited_at <= created_at {
                return Err("GitHub did not return an issue edit newer than the thread creation.".into());
            }
            edited_at
        }
        "check_run.completed" => {
            if text(event_node, "status") != "COMPLETED" {
                return Err("GitHub returned a check run that has not completed.".into());
            }
            let _completed_at = observe_timestamp(event_node, "completedAt")?;
            event_node
                .get("fullDatabaseId")
                .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok()))
                .filter(|id| *id > 0)
                .ok_or("GitHub did not return a stable check run ID.")?
        }
        _ => event_node
            .get("fullDatabaseId")
            .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok()))
            .filter(|id| *id > 0)
            .ok_or("GitHub did not return a stable event ID.")?,
    };
    let actor_path = match derived_kind {
        "issues.edited" => "/editor/login",
        // GitHub exposes the Check Suite's owning App in the native response;
        // its slug is the allowlist identity for this CI event class.
        "check_run.completed" => "/checkSuite/app/slug",
        _ => "/author/login",
    };
    let actor = event_node
        .pointer(actor_path)
        .and_then(serde_json::Value::as_str)
        .filter(|actor| {
            !actor.is_empty()
                && actor.len() <= 80
                && actor.is_ascii()
                && actor.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'[' | b']')
                })
        })
        .ok_or("GitHub did not return a usable event actor.")?
        .to_string();
    let event_slug = derived_kind.replace('.', "-");
    let delivery_id = if derived_kind == "issues.edited" {
        let global_id = event_node
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("GitHub did not return a stable issue edit ID.")?;
        observe_edit_delivery_id(global_id)?
    } else {
        format!("gh-{event_slug}-{sequence}")
    };
    Ok(ObserveEvent {
        repository: actual_repository,
        actor,
        event_kind: derived_kind.into(),
        subject_key: actual_subject,
        delivery_id,
        sequence,
    })
}
/// A spawn failure means the CLI is absent; anything else is reported as is.
fn start_failure(error: &str) -> String {
    if error.starts_with("Could not start") {
        "GitHub CLI (gh) is not installed.".into()
    } else {
        error.to_string()
    }
}

/// Whether `gh` exists, who is signed in, and which repository this checkout uses.
pub fn status(root: &Path) -> GithubStatus {
    let mut status = GithubStatus::default();
    let version = match run_gh(root, &["--version"], Duration::from_secs(10)) {
        Ok(output) => output,
        Err(error) => {
            status.error = Some(start_failure(&error));
            return status;
        }
    };
    status.installed = true;
    status.version = String::from_utf8_lossy(&version.bytes)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches("gh version ")
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string();
    let mut command = gh_base(root);
    command
        .args(["auth", "status"])
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("NO_COLOR", "1");
    match run_process(command, "GitHub CLI", 64 * 1024, Duration::from_secs(20)) {
        Ok(output) => {
            let report = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout.bytes),
                String::from_utf8_lossy(&output.stderr.bytes)
            );
            status.authenticated = output.status.success();
            status.login = report.lines().find_map(|line| {
                let (_, rest) = line.split_once(" account ")?;
                rest.split_whitespace().next().map(str::to_string)
            });
            if !status.authenticated {
                status.error = Some("Sign in with `gh auth login` to connect GitHub.".into());
                return status;
            }
        }
        Err(error) => {
            status.error = Some(error);
            return status;
        }
    }
    match repository(root) {
        Ok(repo) => status.repo = Some(repo),
        Err(error) if error.is_empty() => {}
        Err(error) => status.error = Some(error),
    }
    status
}

/// The GitHub repository behind this checkout's remote; `Err("")` means none.
pub fn repository(root: &Path) -> Result<Repository, String> {
    let output = run_gh(
        root,
        &["repo", "view", "--json", "nameWithOwner,defaultBranchRef,url"],
        TIMEOUT,
    )
    .map_err(|error| {
        let lower = error.to_ascii_lowercase();
        if ["no git remote", "not a git repository", "could not determine", "none of the git remotes", "unable to determine base repository"]
            .iter()
            .any(|marker| lower.contains(marker))
        {
            String::new()
        } else {
            error
        }
    })?;
    let value = json(&output)?;
    let name_with_owner = text(&value, "nameWithOwner");
    if name_with_owner.is_empty() {
        return Err(String::new());
    }
    Ok(Repository {
        name_with_owner,
        default_branch: value
            .get("defaultBranchRef")
            .map(|v| text(v, "name"))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "main".into()),
        url: text(&value, "url"),
    })
}

fn item(kind: &str, value: &serde_json::Value) -> Option<Item> {
    Some(Item {
        kind: kind.into(),
        number: value.get("number")?.as_i64()?,
        title: text(value, "title"),
        state: state_of(value),
        url: text(value, "url"),
        author: value.get("author").map(|a| text(a, "login")).unwrap_or_default(),
    })
}
pub(crate) fn view(root: &Path, kind: &str, number: i64, fields: &str) -> Result<serde_json::Value, String> {
    let command = if kind == "pull" { "pr" } else { "issue" };
    let value = json(&run_gh(root, &[command, "view", &number.to_string(), "--json", fields], TIMEOUT)?)?;
    // The REST-style issue lookup can answer for a pull request; keep the kinds honest.
    if kind == "issue" && text(&value, "url").contains("/pull/") {
        return Err("This number is a pull request.".into());
    }
    Ok(value)
}

/// Open issues and pull requests matching the picker's query.
pub fn search(root: &Path, query: &str, limit: usize) -> Result<Vec<Item>, String> {
    let query = valid_query(query)?;
    let limit = limit.clamp(1, 20).to_string();
    let mut items = Vec::new();
    if let Ok(number) = query.trim_start_matches('#').parse::<i64>() {
        if number > 0 {
            for kind in ["issue", "pull"] {
                if let Ok(value) = view(root, kind, number, "number,title,state,url,author,isDraft") {
                    items.extend(item(kind, &value));
                }
            }
            if !items.is_empty() {
                return Ok(items);
            }
        }
    }
    for (kind, command) in [("issue", "issue"), ("pull", "pr")] {
        let mut args = vec![command, "list", "--state", "open", "--limit", &limit, "--json", "number,title,state,url,author,isDraft"];
        let search = flag("search", &query);
        if !query.is_empty() {
            args.push(&search);
        }
        let value = json(&run_gh(root, &args, TIMEOUT)?)?;
        items.extend(value.as_array().into_iter().flatten().filter_map(|v| item(kind, v)));
    }
    Ok(items)
}

/// Open issues for the board import dialog.
pub fn issues(root: &Path, filter: &IssueFilter) -> Result<Vec<Issue>, String> {
    let mut args: Vec<String> = ["issue", "list", "--state", "open", "--limit", "50", "--json", "number,title,body,state,url,labels,assignees,updatedAt"]
        .into_iter()
        .map(str::to_string)
        .collect();
    if filter.assigned_to_me {
        args.push(flag("assignee", "@me"));
    }
    if let Some(label) = filter.label.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        if label.len() > 100 || label.contains('\0') {
            return Err("Shorten the label filter.".into());
        }
        args.push(flag("label", label));
    }
    if let Some(query) = filter.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        args.push(flag("search", &valid_query(query)?));
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let value = json(&run_gh(root, &borrowed, TIMEOUT)?)?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let (body, _) = excerpt(&text(v, "body"), IMPORT_BODY_LIMIT);
            Some(Issue {
                number: v.get("number")?.as_i64()?,
                title: text(v, "title"),
                body,
                state: state_of(v),
                url: text(v, "url"),
                labels: names(v, "labels", "name"),
                assignees: names(v, "assignees", "login"),
                updated_at: text(v, "updatedAt"),
            })
        })
        .collect())
}

/// Full references for `#123` mentions, resolved in parallel and bounded.
pub fn resolve(root: &Path, numbers: &[i64]) -> Result<Vec<Reference>, String> {
    let mut wanted: Vec<i64> = Vec::new();
    for number in numbers {
        if *number > 0 && !wanted.contains(number) {
            wanted.push(*number);
        }
        if wanted.len() == MAX_REFERENCES {
            break;
        }
    }
    let results: Vec<Option<Reference>> = std::thread::scope(|scope| {
        let handles: Vec<_> = wanted
            .iter()
            .map(|number| {
                let number = *number;
                scope.spawn(move || {
                    for kind in ["issue", "pull"] {
                        if let Ok(value) = view(root, kind, number, "number,title,state,url,author,body,labels,isDraft") {
                            let (body, truncated) = excerpt(&text(&value, "body"), BODY_LIMIT);
                            return Some(Reference {
                                kind: kind.into(),
                                number,
                                title: text(&value, "title"),
                                state: state_of(&value),
                                url: text(&value, "url"),
                                author: value.get("author").map(|a| text(a, "login")).unwrap_or_default(),
                                body,
                                labels: names(&value, "labels", "name"),
                                truncated,
                            });
                        }
                    }
                    None
                })
            })
            .collect();
        handles.into_iter().map(|handle| handle.join().unwrap_or(None)).collect()
    });
    Ok(results.into_iter().flatten().collect())
}

/// One GraphQL round trip for every linked number in a project.
pub fn link_states(root: &Path, repo: &Repository, links: &[(String, i64)]) -> Result<Vec<LinkState>, String> {
    let (owner, name) = repo
        .name_with_owner
        .split_once('/')
        .ok_or("The repository name is incomplete.")?;
    let mut numbers: Vec<i64> = links.iter().map(|(_, number)| *number).filter(|n| *n > 0).collect();
    numbers.sort_unstable();
    numbers.dedup();
    numbers.truncate(100);
    if numbers.is_empty() {
        return Ok(vec![]);
    }
    let fields = numbers
        .iter()
        .map(|number| format!("n{number}: issueOrPullRequest(number: {number}) {{ __typename ... on Issue {{ number title state url }} ... on PullRequest {{ number title state url isDraft }} }}"))
        .collect::<Vec<_>>()
        .join(" ");
    let query = format!("query($owner: String!, $name: String!) {{ repository(owner: $owner, name: $name) {{ {fields} }} }}");
    let owner_flag = flag("raw-field", &format!("owner={owner}"));
    let name_flag = flag("raw-field", &format!("name={name}"));
    let query_flag = flag("raw-field", &format!("query={query}"));
    // One deleted or transferred number makes gh exit 1 while still returning every
    // other node, so the data is read before the exit status is judged.
    let output = run_process(gh_command(root, &["api", "graphql", &owner_flag, &name_flag, &query_flag]), "GitHub CLI", OUTPUT_LIMIT, TIMEOUT)?;
    let value = match json(&output.stdout) {
        Ok(value) if value.pointer("/data/repository").is_some() => value,
        _ => return checked(output).and_then(|capture| json(&capture)).map(|_| vec![]),
    };
    let repository = value.pointer("/data/repository").cloned().unwrap_or(serde_json::Value::Null);
    Ok(numbers
        .iter()
        .filter_map(|number| {
            let node = repository.get(format!("n{number}"))?;
            let kind = match text(node, "__typename").as_str() {
                "Issue" => "issue",
                "PullRequest" => "pull",
                _ => return None,
            };
            Some(LinkState {
                kind: kind.into(),
                number: *number,
                state: state_of(node),
                title: text(node, "title"),
                url: text(node, "url"),
            })
        })
        .collect())
}

/// Open a pull request for an already pushed branch.
pub fn create_pull_request(root: &Path, title: &str, body: &str, base: &str, head: &str) -> Result<PullRequest, String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 256 {
        return Err("Give the pull request a title of up to 256 characters.".into());
    }
    if body.len() > 64 * 1024 {
        return Err("Shorten the pull request description.".into());
    }
    for reference in [base, head] {
        if reference.is_empty() || reference.len() > 200 || reference.starts_with('-') || reference.contains("..") {
            return Err("Choose valid base and head branches.".into());
        }
    }
    let output = run_gh(
        root,
        &["pr", "create", &flag("title", title), &flag("body", body), &flag("base", base), &flag("head", head)],
        Duration::from_secs(60),
    )?;
    let url = String::from_utf8_lossy(&output.bytes)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with("https://"))
        .ok_or("GitHub did not return the pull request URL.")?
        .to_string();
    let number = url
        .rsplit('/')
        .next()
        .and_then(|segment| segment.parse().ok())
        .ok_or("GitHub returned an unexpected pull request URL.")?;
    Ok(PullRequest {
        number,
        url,
        title: title.to_string(),
        state: "open".into(),
    })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn gh_runs_with_the_tool_search_path_even_from_a_minimal_gui_path() {
        // Finder and the Dock start apps with PATH=/usr/bin:/bin; gh must still find Homebrew.
        let command = super::gh_command(std::path::Path::new("/"), &["--version"]);
        let path = command.get_envs().find(|(key, _)| *key == "PATH").and_then(|(_, value)| value).expect("gh gets a PATH");
        let dirs: Vec<_> = std::env::split_paths(path).collect();
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            assert!(dirs.contains(&std::path::PathBuf::from(dir)), "{dir} is searched");
        }
    }

    use super::*;

    #[test]
    fn excerpts_cut_on_character_boundaries_and_flag_truncation() {
        let body = "héllo wörld".repeat(500);
        let (short, truncated) = excerpt(&body, 4096);
        assert!(truncated);
        assert!(short.len() <= 4096);
        assert!(body.starts_with(&short));
        assert_eq!(excerpt("small", 4096), ("small".into(), false));
    }

    #[test]
    fn items_read_gh_json_and_lower_case_states() {
        let value: serde_json::Value = serde_json::from_str(r#"{"number":12,"title":"Login bug","state":"OPEN","url":"https://github.com/o/r/issues/12","author":{"login":"octocat"},"labels":[{"name":"bug"}]}"#).unwrap();
        let parsed = item("issue", &value).unwrap();
        assert_eq!(parsed.number, 12);
        assert_eq!(parsed.state, "open");
        assert_eq!(parsed.author, "octocat");
        assert_eq!(names(&value, "labels", "name"), vec!["bug"]);
        let draft: serde_json::Value = serde_json::from_str(r#"{"number":3,"state":"OPEN","isDraft":true,"url":"","title":""}"#).unwrap();
        assert_eq!(state_of(&draft), "draft");
        assert!(item("pull", &serde_json::json!({"title":"missing number"})).is_none());
    }

    #[test]
    fn user_text_travels_as_flag_values_and_is_bounded() {
        assert_eq!(flag("search", "-rf --exec"), "--search=-rf --exec");
        assert!(valid_query(&"x".repeat(201)).is_err());
        assert_eq!(valid_query("  login bug ").unwrap(), "login bug");
    }

    #[test]
    fn observe_issues_edited_uses_only_latest_edit_identity_actor_and_time() {
        let payload = serde_json::json!({
            "data": { "repository": { "nameWithOwner": "test/fixture",
                "issueOrPullRequest": { "__typename": "Issue", "number": 42,
                    "createdAt": "2025-02-03T04:00:00Z",
                    "userContentEdits": { "nodes": [{
                        "id": "VXNlckNvbnRlbnRFZGl0OjEyMw==",
                        "editedAt": "2025-02-03T04:05:06.789Z",
                        "editor": { "login": "trusted-editor" }
                    }] }
                }
            } }
        }).to_string();
        let mut calls = Vec::new();
        let event = observe_event_with("test/fixture", "issue:42", "issues.edited", |args, limit, timeout| {
            assert!(limit <= OBSERVE_OUTPUT_LIMIT);
            assert!(timeout <= TIMEOUT);
            calls.push(args.to_vec());
            if args.first().map(String::as_str) == Some("auth") {
                Ok(Capture { bytes: b"Logged in".to_vec(), truncated: false })
            } else {
                Ok(Capture { bytes: payload.as_bytes().to_vec(), truncated: false })
            }
        }).unwrap();

        assert!(supports_observe_event("issues.edited"));
        assert_eq!(event.event_kind, "issues.edited");
        assert_eq!(event.actor, "trusted-editor");
        assert_eq!(event.subject_key, "issue:42");
        assert_eq!(event.sequence,
            chrono::DateTime::parse_from_rfc3339("2025-02-03T04:05:06.789Z").unwrap().timestamp_millis());
        assert_eq!(event.delivery_id, observe_edit_delivery_id("VXNlckNvbnRlbnRFZGl0OjEyMw==").unwrap());
        assert!(event.delivery_id.starts_with("gh-issues-edited-"));
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], vec!["auth", "status", "--hostname=github.com"]);
        let query = calls[1].join(" ");
        assert!(query.contains("userContentEdits") && query.contains("editedAt") && query.contains("editor { login }"));
        assert!(!query.contains("title") && !query.contains("body") && !query.contains("diff"));
    }

    #[test]
    fn observe_check_run_completed_is_pull_bound_and_requires_completed_native_metadata() {
        let payload = serde_json::json!({
            "data": { "repository": { "nameWithOwner": "test/fixture",
                "issueOrPullRequest": { "__typename": "PullRequest", "number": 42,
                    "commits": { "nodes": [{ "commit": { "checkSuites": { "nodes": [{
                        "checkRuns": { "nodes": [{
                            "fullDatabaseId": 9876,
                            "status": "COMPLETED",
                            "completedAt": "2025-02-03T04:05:06Z",
                            "checkSuite": { "app": { "slug": "trusted-checker" } }
                        }] }
                    }] } } }] }
                }
            } }
        }).to_string();
        let mut calls = Vec::new();
        let event = observe_event_with("test/fixture", "pull:42", "check_run.completed", |args, limit, timeout| {
            assert!(limit <= OBSERVE_OUTPUT_LIMIT);
            assert!(timeout <= TIMEOUT);
            calls.push(args.to_vec());
            if args.first().map(String::as_str) == Some("auth") {
                Ok(Capture { bytes: b"Logged in".to_vec(), truncated: false })
            } else {
                Ok(Capture { bytes: payload.as_bytes().to_vec(), truncated: false })
            }
        }).unwrap();

        assert!(supports_observe_event("check_run.completed"));
        assert_eq!(event.event_kind, "check_run.completed");
        assert_eq!(event.actor, "trusted-checker");
        assert_eq!(event.subject_key, "pull:42");
        assert_eq!(event.delivery_id, "gh-check_run-completed-9876");
        assert_eq!(event.sequence, 9876);
        assert_eq!(calls.len(), 2);
        let query = calls[1].join(" ");
        assert!(query.contains("checkSuites") && query.contains("checkRuns") && query.contains("app { slug }"));
        assert!(!query.contains("title") && !query.contains("body") && !query.contains("conclusion"));

        let pending_payload = serde_json::json!({
            "data": { "repository": { "nameWithOwner": "test/fixture",
                "issueOrPullRequest": { "__typename": "PullRequest", "number": 42,
                    "commits": { "nodes": [{ "commit": { "checkSuites": { "nodes": [{
                        "checkRuns": { "nodes": [{
                            "fullDatabaseId": 9877, "status": "IN_PROGRESS",
                            "completedAt": null,
                            "checkSuite": { "app": { "slug": "trusted-checker" } }
                        }] }
                    }] } } }] }
                }
            } }
        }).to_string();
        let mut pending_calls = 0;
        assert!(observe_event_with("test/fixture", "pull:42", "check_run.completed", |args, _, _| {
            pending_calls += 1;
            if args.first().map(String::as_str) == Some("auth") {
                Ok(Capture { bytes: b"Logged in".to_vec(), truncated: false })
            } else {
                Ok(Capture { bytes: pending_payload.as_bytes().to_vec(), truncated: false })
            }
        }).is_err());
        assert_eq!(pending_calls, 2, "an unfinished check run is rejected after one bounded read");

        let mut wrong_thread_calls = 0;
        assert!(observe_event_with("test/fixture", "issue:42", "check_run.completed", |_, _, _| {
            wrong_thread_calls += 1;
            Err("unexpected GitHub call".into())
        }).is_err());
        assert_eq!(wrong_thread_calls, 0, "a check run source cannot fetch an issue thread");
        assert!(!supports_observe_event("check_suite.completed"));
    }

    /// Read-only check against the real CLI; run with `cargo test -- --ignored github::` when gh is signed in.
    #[test]
    #[ignore]
    fn live_cli_shapes_match_the_parsers() {
        let root = std::env::current_dir().unwrap();
        let status = status(&root);
        assert!(status.installed && status.authenticated, "{status:?}");
        let repo = status.repo.expect("this checkout has a GitHub remote");
        assert!(!repo.default_branch.is_empty());
        let states = link_states(&root, &Repository { name_with_owner: "cli/cli".into(), default_branch: "trunk".into(), url: String::new() }, &[("issue".into(), 1), ("pull".into(), 999_999_999)]).unwrap();
        assert_eq!(states.len(), 1, "a missing number is skipped, not fatal: {states:?}");
        assert_eq!(states[0].number, 1);
        assert!(["issue", "pull"].contains(&states[0].kind.as_str()));
    }

    #[test]
    fn a_missing_cli_is_named_and_other_failures_pass_through() {
        assert_eq!(start_failure("Could not start GitHub CLI: No such file or directory"), "GitHub CLI (gh) is not installed.");
        assert_eq!(start_failure("GitHub CLI operation timed out. Check the repository before retrying."), "GitHub CLI operation timed out. Check the repository before retrying.");
    }
}
