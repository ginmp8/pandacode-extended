//! GitHub through the user's own `gh` CLI. PandaCode stores no tokens: every call
//! runs the signed-in CLI with a null stdin, bounded output, and a deadline, and
//! only ever from a project's checkout so the repository is the remote's.
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, sync::{Mutex, MutexGuard, OnceLock}, time::Duration};

use crate::workspace::{checked, run_process, Capture};

pub(crate) const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const BODY_LIMIT: usize = 4 * 1024;
const IMPORT_BODY_LIMIT: usize = 16 * 1024;
pub const MAX_REFERENCES: usize = 5;
pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);

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
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedIssueStatus {
    pub session_id: String,
    pub link: crate::storage::TaskLink,
    pub repository: String,
    pub remote_state: String,
    pub needs_action: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedIssueActionFailure {
    pub session_id: String,
    pub action: String,
    pub repository: String,
    pub number: i64,
    pub error: String,
}

static ISSUE_ACTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn issue_action_lock() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    ISSUE_ACTION_LOCK.get_or_init(|| Mutex::new(())).lock().map_err(|e| e.to_string())
}

fn issue_action_for_stage(stage: &str) -> Option<&'static str> {
    match stage { "completed" => Some("close"), "review" | "pending" => Some("reopen"), _ => None }
}

fn issue_store(store: &Mutex<crate::storage::Store>) -> Result<MutexGuard<'_, crate::storage::Store>, String> {
    store.lock().map_err(|e| e.to_string())
}

fn canonical_issue_url(repository: &str, number: i64) -> Result<String, String> {
    if number <= 0 { return Err("The linked issue number is invalid.".into()); }
    let (owner, name) = repository.split_once('/').ok_or("The project repository name is incomplete.")?;
    let valid_segment = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !valid_segment(owner) || !valid_segment(name) || name.contains('/') {
        return Err("The project repository name is invalid.".into());
    }
    Ok(format!("https://github.com/{owner}/{name}/issues/{number}"))
}

fn repository_from_issue_url(url: &str, number: i64) -> Result<String, String> {
    let suffix = format!("/issues/{number}");
    let repo_path = url.strip_prefix("https://github.com/").and_then(|value| value.strip_suffix(&suffix)).ok_or("The linked issue URL is not canonical.")?;
    let canonical = canonical_issue_url(repo_path, number)?;
    if canonical != url { return Err("The linked issue URL is not canonical.".into()); }
    Ok(repo_path.to_string())
}

fn issue_action_args(action: &str, number: i64, repository: &str) -> Result<Vec<String>, String> {
    if !["close", "reopen"].contains(&action) { return Err("Choose whether to close or reopen the issue.".into()); }
    canonical_issue_url(repository, number)?;
    let mut args = vec!["issue".to_string(), action.to_string(), number.to_string(), flag("repo", repository)];
    if action == "close" { args.push("--reason=completed".into()); }
    Ok(args)
}

fn linked_issue_identity(root: &Path, task: &crate::storage::Session) -> Result<(crate::storage::TaskLink, Repository), String> {
    if task.remote_device.is_some() { return Err("This task runs on another computer.".into()); }
    let link = task.issue.clone().ok_or("This task has no linked GitHub issue.")?;
    if link.kind != "issue" || link.number <= 0 { return Err("The linked GitHub item is not a valid issue.".into()); }
    let repo = repository(root).map_err(|e| if e.is_empty() { "This project has no GitHub remote.".to_string() } else { e })?;
    let expected = canonical_issue_url(&repo.name_with_owner, link.number)?;
    if link.url != expected { return Err("The linked issue URL does not match this project's repository.".into()); }
    Ok((link, repo))
}

fn read_linked_issue(root: &Path, link: &crate::storage::TaskLink, repo: &Repository) -> Result<serde_json::Value, String> {
    let canonical = canonical_issue_url(&repo.name_with_owner, link.number)?;
    if link.url != canonical { return Err("The linked issue URL does not match this project's repository.".into()); }
    let number = link.number.to_string();
    let repo_arg = flag("repo", &repo.name_with_owner);
    let value = json(&run_gh(root, &["issue", "view", &number, &repo_arg, "--json", "number,url,state,title"], TIMEOUT)?)?;
    if value.get("number").and_then(|v| v.as_i64()) != Some(link.number)
        || text(&value, "url") != canonical
        || text(&value, "url").contains("/pull/")
    { return Err("GitHub returned a different item than the linked issue.".into()); }
    let state = text(&value, "state").to_ascii_lowercase();
    if !matches!(state.as_str(), "open" | "closed") { return Err("GitHub returned an unexpected issue state.".into()); }
    Ok(value)
}

pub fn linked_issue_status(root: &Path, task: &crate::storage::Session, action: &str) -> Result<LinkedIssueStatus, String> {
    if !["close", "reopen"].contains(&action) || issue_action_for_stage(&task.task_stage) != Some(action) {
        return Err("This action does not match the task's current stage.".into());
    }
    let (link, repo) = linked_issue_identity(root, task)?;
    let value = read_linked_issue(root, &link, &repo)?;
    let remote_state = text(&value, "state").to_ascii_lowercase();
    let needs_action = match action { "close" if remote_state != "closed" => Some("close".into()), "reopen" if remote_state != "open" => Some("reopen".into()), _ => None };
    Ok(LinkedIssueStatus { session_id: task.id.clone(), link, repository: repo.name_with_owner, remote_state, needs_action })
}

fn apply_saved_issue_action(root: &Path, store: &Mutex<crate::storage::Store>, sid: &str) -> Result<crate::storage::Session, String> {
    let (action, task) = {
        let store = issue_store(store)?;
        (store.github_issue_action(sid)?.ok_or("There is no approved linked issue action to apply.")?, store.session(sid)?)
    };
    let linked = task.issue.as_ref().ok_or("This task no longer has a linked GitHub issue.")?;
    if issue_action_for_stage(&task.task_stage) != Some(action.action.as_str()) || linked.kind != "issue" || linked.number != action.number || linked.url != action.url {
        issue_store(store)?.cancel_github_issue_action(sid)?;
        return Err("The task stage or linked issue changed; approve the action again.".into());
    }
    let (link, repo) = linked_issue_identity(root, &task)?;
    if repo.name_with_owner != action.repository {
        issue_store(store)?.cancel_github_issue_action(sid)?;
        return Err("The linked issue no longer matches this project's repository.".into());
    }
    let current = read_linked_issue(root, &link, &repo)?;
    let desired_state = if action.action == "close" { "closed" } else { "open" };
    if text(&current, "state").eq_ignore_ascii_case(desired_state) {
        let store = issue_store(store)?;
        store.finish_github_issue_action(sid, &action.action, &action.repository, action.number, &action.url, desired_state, &text(&current, "title"))?;
        return store.session(sid);
    }
    {
        let store = issue_store(store)?;
        let latest = store.github_issue_action(sid)?.ok_or("The approved linked issue action was canceled.")?;
        let latest_task = store.session(sid)?;
        let latest_link = latest_task.issue.as_ref().ok_or("This task no longer has a linked GitHub issue.")?;
        if latest.action != action.action || latest.repository != action.repository || latest.number != action.number || latest.url != action.url
            || issue_action_for_stage(&latest_task.task_stage) != Some(latest.action.as_str())
            || latest_link.number != latest.number || latest_link.url != latest.url || latest_link.kind != "issue"
        { return Err("The approved linked issue action is no longer current.".into()); }
    }
    let args = issue_action_args(&action.action, action.number, &action.repository)?;
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_gh(root, &args, TIMEOUT)?;
    let verified = read_linked_issue(root, &link, &repo)?;
    if !text(&verified, "state").eq_ignore_ascii_case(desired_state) {
        return Err("GitHub did not confirm the approved issue state change.".into());
    }
    let store = issue_store(store)?;
    store.finish_github_issue_action(sid, &action.action, &action.repository, action.number, &action.url, desired_state, &text(&verified, "title"))?;
    store.session(sid)
}

pub fn apply_linked_issue_action(root: &Path, store: &Mutex<crate::storage::Store>, sid: &str, action: &str) -> Result<crate::storage::Session, String> {
    let _guard = issue_action_lock()?;
    if !["close", "reopen"].contains(&action) { return Err("Choose whether to close or reopen the issue.".into()); }
    let store_lock = issue_store(store)?;
    let task = store_lock.session(sid)?;
    let expected = issue_action_for_stage(&task.task_stage);
    if expected != Some(action) { return Err("This action does not match the task's current stage.".into()); }
    let link = task.issue.as_ref().ok_or("This task has no linked GitHub issue.")?;
    if link.kind != "issue" || link.number <= 0 { return Err("The linked GitHub item is not a valid issue.".into()); }
    let repo = repository_from_issue_url(&link.url, link.number)?;
    store_lock.approve_github_issue_action(sid, action, &repo, link.number, &link.url)?;
    drop(store_lock);
    apply_saved_issue_action(root, store, sid)
}

pub fn cancel_linked_issue_action(store: &Mutex<crate::storage::Store>, sid: &str) -> Result<(), String> {
    issue_store(store)?.cancel_github_issue_action(sid)
}

fn retry_failure_if_pending(store: &Mutex<crate::storage::Store>, expected: &crate::storage::GithubIssueAction, error: String) -> Result<Option<LinkedIssueActionFailure>, String> {
    let store = issue_store(store)?;
    let Some(current) = store.github_issue_action(&expected.session_id)? else { return Ok(None); };
    if current.action != expected.action || current.repository != expected.repository || current.number != expected.number || current.url != expected.url {
        return Ok(None);
    }
    let task = store.session(&expected.session_id)?;
    let valid = issue_action_for_stage(&task.task_stage) == Some(current.action.as_str())
        && task.remote_device.is_none()
        && task.issue.as_ref().is_some_and(|link| link.kind == "issue" && link.number == current.number && link.url == current.url)
        && repository_from_issue_url(&current.url, current.number).is_ok_and(|repo| repo == current.repository);
    if !valid {
        store.cancel_github_issue_action(&expected.session_id)?;
        return Ok(None);
    }
    Ok(Some(LinkedIssueActionFailure {
        session_id: current.session_id,
        action: current.action,
        repository: current.repository,
        number: current.number,
        error,
    }))
}

pub fn retry_linked_issue_actions(root: &Path, store: &Mutex<crate::storage::Store>, project_id: &str) -> Result<Vec<LinkedIssueActionFailure>, String> {
    let actions = issue_store(store)?.project_github_issue_actions(project_id)?;
    let mut failures = Vec::new();
    for action in actions {
        let _guard = issue_action_lock()?;
        if let Err(error) = apply_saved_issue_action(root, store, &action.session_id) {
            if let Some(failure) = retry_failure_if_pending(store, &action, error)? { failures.push(failure); }
        }
    }
    Ok(failures)
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
    checked(run_process(gh_command(root, args), "GitHub CLI", OUTPUT_LIMIT, timeout)?)
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
    use super::{canonical_issue_url, issue_action_args, issue_action_for_stage, repository_from_issue_url, retry_failure_if_pending};
    use crate::storage::{Store, TaskLink};
    use std::{path::Path, sync::Mutex};

    #[test]
    fn linked_issue_action_validation_is_canonical_and_stage_scoped() {
        assert_eq!(canonical_issue_url("owner/repo", 11).unwrap(), "https://github.com/owner/repo/issues/11");
        assert_eq!(repository_from_issue_url("https://github.com/owner/repo/issues/11", 11).unwrap(), "owner/repo");
        assert!(repository_from_issue_url("https://github.com/owner/repo/pull/11", 11).is_err());
        for (repo, number) in [("owner/repo/extra", 11), ("owner/repo", 0), ("https://github.com/owner/repo", 11)] {
            assert!(canonical_issue_url(repo, number).is_err());
        }
        assert_eq!(issue_action_for_stage("completed"), Some("close"));
        assert_eq!(issue_action_for_stage("review"), Some("reopen"));
        assert_eq!(issue_action_for_stage("pending"), Some("reopen"));
        assert_eq!(issue_action_for_stage("in_progress"), None);
    }

    #[test]
    fn linked_issue_action_arguments_are_exact_and_validated() {
        assert_eq!(issue_action_args("close", 11, "owner/repo").unwrap(), vec![String::from("issue"), String::from("close"), String::from("11"), String::from("--repo=owner/repo"), String::from("--reason=completed")]);
        assert_eq!(issue_action_args("reopen", 11, "owner/repo").unwrap(), vec![String::from("issue"), String::from("reopen"), String::from("11"), String::from("--repo=owner/repo")]);
        assert!(issue_action_args("delete", 11, "owner/repo").is_err());
        assert!(issue_action_args("close", 0, "owner/repo").is_err());
        assert!(issue_action_args("close", 11, "owner/repo/extra").is_err());
    }

    #[test]
    fn canceled_issue_approval_is_removed_without_network_access() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let project = store.add_project(&std::env::temp_dir()).unwrap();
        let task = store.create_session(&project.id, "codex", "default", "ask").unwrap();
        let link = TaskLink { kind:"issue".into(), number:11, url:"https://github.com/owner/repo/issues/11".into(), title:"Test".into(), state:"open".into(), checked_at:0 };
        store.set_task_link(&task.id, "issue", Some(&link)).unwrap();
        store.set_task_stage(&task.id, "completed").unwrap();
        store.approve_github_issue_action(&task.id, "close", "owner/repo", 11, &link.url).unwrap();
        assert_eq!(store.github_issue_action(&task.id).unwrap().unwrap().action, "close");
        let other = store.create_session(&project.id, "codex", "default", "ask").unwrap();
        store.set_task_link(&other.id, "issue", Some(&link)).unwrap();
        store.set_task_stage(&other.id, "completed").unwrap();
        store.approve_github_issue_action(&other.id, "close", "owner/repo", 11, &link.url).unwrap();
        assert!(store.github_issue_action(&task.id).unwrap().is_none());
        assert!(store.github_issue_action(&other.id).unwrap().is_some());
        store.cancel_github_issue_action(&task.id).unwrap();
        assert!(store.github_issue_action(&other.id).unwrap().is_some());
        store.set_task_stage(&other.id, "review").unwrap();
        assert!(store.github_issue_action(&other.id).unwrap().is_none());
    }

    #[test]
    fn retry_failure_is_returned_only_while_approval_remains_pending() {
        let store = Mutex::new(Store::open(Path::new(":memory:")).unwrap());
        let project = store.lock().unwrap().add_project(&std::env::temp_dir()).unwrap();
        let task = store.lock().unwrap().create_session(&project.id, "codex", "default", "ask").unwrap();
        let link = TaskLink { kind:"issue".into(), number:11, url:"https://github.com/owner/repo/issues/11".into(), title:"Test".into(), state:"open".into(), checked_at:0 };
        {
            let store = store.lock().unwrap();
            store.set_task_link(&task.id, "issue", Some(&link)).unwrap();
            store.set_task_stage(&task.id, "completed").unwrap();
            store.approve_github_issue_action(&task.id, "close", "owner/repo", 11, &link.url).unwrap();
        }
        let pending = store.lock().unwrap().github_issue_action(&task.id).unwrap().unwrap();
        let failure = retry_failure_if_pending(&store, &pending, "temporary failure".into()).unwrap().unwrap();
        assert_eq!(failure.session_id, task.id);
        assert_eq!(failure.action, "close");
        assert_eq!(failure.repository, "owner/repo");
        assert_eq!(failure.number, 11);
        assert_eq!(failure.error, "temporary failure");
        store.lock().unwrap().cancel_github_issue_action(&task.id).unwrap();
        assert!(retry_failure_if_pending(&store, &pending, "temporary failure".into()).unwrap().is_none());
    }

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
