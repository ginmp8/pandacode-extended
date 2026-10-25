//! Hash-pinned bot/project selections. They never mutate a task's checkout: ordinary
//! harness activations retain their native folders, while scoped additions use bounded text.
use std::{collections::BTreeMap, fs, path::Path};
use serde::{Deserialize, Serialize};
use super::{active_in, content_hash, frontmatter, Kind, Library, Scope, MAX_BYTES};

const MAX_SELECTION: usize = 32;
const ITEM_BYTES: usize = 8 * 1024;
const CONTEXT_BYTES: usize = 32 * 1024;
const HEADER: &str = "\n\nSelected skills (user-approved context; existing permissions still apply):\n";
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target { Bot, Project }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pin { pub id: String, pub sha256: String }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Selections {
    #[serde(default)] bots: BTreeMap<String, Vec<Pin>>,
    #[serde(default)] projects: BTreeMap<String, Vec<Pin>>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved { pub id: String, pub sha256: String, pub name: String, pub provenance: String, pub delivery: String, pub reason: Option<String> }
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution { pub items: Vec<Resolved>, #[serde(skip_serializing)] pub text: String }

/// Stable union, with actual origin retained. Duplicates within one source stay that source.
pub(super) fn union(bot: &[Pin], project: &[Pin]) -> Vec<(Pin, &'static str)> {
    let mut result: Vec<(Pin, &'static str)> = Vec::new();
    for (pins, origin) in [(bot, "bot"), (project, "project")] {
        for pin in pins {
            match result.iter_mut().find(|(p, _)| p.id == pin.id) {
                Some((_, provenance)) if *provenance != origin => *provenance = "both",
                Some(_) => {},
                None => result.push((pin.clone(), origin)),
            }
        }
    }
    result
}
fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max { return text.into(); }
    let mut end = max.saturating_sub(32);
    while !text.is_char_boundary(end) { end -= 1; }
    format!("{}\n[Skill text truncated]\n", &text[..end])
}
impl Library {
    fn load_selections(&self) -> Result<Selections, String> {
        match fs::read(self.dir.join(".scopes.json")) {
            Ok(bytes) if bytes.len() <= MAX_BYTES => serde_json::from_slice(&bytes).map_err(|e| format!("Skill selections are damaged: {e}")),
            Ok(_) => Err("Skill selections exceed the size limit.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Selections::default()),
            Err(e) => Err(format!("Could not read skill selections: {e}")),
        }
    }
    pub fn selection(&self, target: Target, id: &str) -> Result<Vec<Pin>, String> {
        let selections = self.load_selections()?;
        Ok(match target { Target::Bot => selections.bots, Target::Project => selections.projects }.get(id).cloned().unwrap_or_default())
    }
    pub fn select(&self, target: Target, target_id: &str, ids: &[String], acknowledge: bool) -> Result<Vec<Pin>, String> {
        let _guard = super::LOCK.lock().map_err(|e| e.to_string())?;
        if ids.len() > MAX_SELECTION { return Err("Choose at most 32 skills per scope.".into()); }
        let mut pins = Vec::new(); let mut metas = Vec::new();
        for id in ids {
            if pins.iter().any(|p: &Pin| p.id == *id) { continue; }
            let item = self.item(self.load(id)?)?;
            if item.meta.kind != Kind::Skill { return Err("Only skills can be selected for a bot or project.".into()); }
            if content_hash(&self.files(id)?) != item.meta.sha256 { return Err(format!("{} was edited on disk; re-import it under a new name first.", item.meta.name)); }
            let mut meta = item.meta;
            if !item.findings.is_empty() && meta.acknowledged.as_deref() != Some(&meta.sha256) {
                if !acknowledge { return Err(format!("Review the safety findings for {} before assigning it.", meta.name)); }
                meta.acknowledged = Some(meta.sha256.clone());
            }
            pins.push(Pin { id: id.clone(), sha256: meta.sha256.clone() }); metas.push(meta);
        }
        let mut selections = self.load_selections()?;
        let targets = match target { Target::Bot => &mut selections.bots, Target::Project => &mut selections.projects };
        if pins.is_empty() { targets.remove(target_id); } else { targets.insert(target_id.into(), pins.clone()); }
        let bytes = serde_json::to_vec_pretty(&selections).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES { return Err("Skill selections exceed the size limit.".into()); }
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        // Acknowledgement is explicit and hash-specific. If selection persistence fails,
        // no scope is enabled by these acknowledgements alone.
        for meta in &metas { self.save(meta)?; }
        let temp = self.dir.join(".scopes.json.tmp");
        fs::write(&temp, bytes).and_then(|_| fs::rename(&temp, self.dir.join(".scopes.json"))).map_err(|e| e.to_string())?;
        Ok(pins)
    }
    pub(super) fn assigned(&self, id: &str) -> Result<bool, String> {
        let selections = self.load_selections()?;
        Ok(selections.bots.values().chain(selections.projects.values()).any(|pins| pins.iter().any(|p| p.id == id)))
    }
    /// The caller holds the Store lock before taking the library lock, matching task
    /// admission and scope writes. Database errors and unavailable folders are not absence.
    fn prune_missing_selections(&self, store: &crate::storage::Store) -> Result<(), String> {
        let _guard = super::LOCK.lock().map_err(|e| e.to_string())?;
        let mut selections = self.load_selections()?;
        let mut changed = false;
        for (targets, query) in [
            (&mut selections.bots, "SELECT EXISTS(SELECT 1 FROM bots WHERE id=?1)"),
            (&mut selections.projects, "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)"),
        ] {
            let mut absent = Vec::new();
            for id in targets.keys() {
                let exists: bool = store.db.query_row(query, [id], |row| row.get(0)).map_err(|e| format!("Could not check skill selection targets: {e}"))?;
                if !exists { absent.push(id.clone()); }
            }
            for id in absent { targets.remove(&id); changed = true; }
        }
        if changed {
            let bytes = serde_json::to_vec_pretty(&selections).map_err(|e| e.to_string())?;
            if bytes.len() > MAX_BYTES { return Err("Skill selections exceed the size limit.".into()); }
            let temp = self.dir.join(".scopes.json.tmp");
            fs::write(&temp, bytes).and_then(|_| fs::rename(&temp, self.dir.join(".scopes.json"))).map_err(|e| format!("Could not remove obsolete skill selections: {e}"))?;
        }
        Ok(())
    }
    /// Explicit removal cleans pins for deleted targets only. Live selections still block
    /// removal; the caller retains the Store lock across cleanup and the existing remove.
    pub(crate) fn remove_with_store(&self, store: &crate::storage::Store, id: &str) -> Result<Vec<super::Item>, String> {
        self.prune_missing_selections(store)?;
        self.remove(id)
    }
    pub fn resolve(&self, harness: &str, root: &Path, project_id: &str, bot_id: Option<&str>) -> Result<Resolution, String> {
        let _guard = super::LOCK.lock().map_err(|e| e.to_string())?;
        let selections = self.load_selections()?;
        let bot = bot_id.and_then(|id| selections.bots.get(id)).cloned().unwrap_or_default();
        let mut project = selections.projects.get(project_id).cloned().unwrap_or_default();
        let metas = self.metadata_entries()?;
        let path = Some(root.to_string_lossy().into_owned());
        for meta in &metas {
            if meta.kind == Kind::Skill && !project.iter().any(|p| p.id == meta.id) {
                if let Some(a) = active_in(meta, harness, Scope::Project, &path) { project.push(Pin { id: meta.id.clone(), sha256: a.sha256.clone() }); }
            }
        }
        let mut selected = union(&bot, &project);
        // Existing prompt activations become operational for every supported harness.
        for meta in &metas {
            if !selected.iter().any(|(p, _)| p.id == meta.id) {
                if let Some(a) = active_in(meta, harness, Scope::User, &None).filter(|a| a.surface == "prompt") {
                    selected.push((Pin { id: meta.id.clone(), sha256: a.sha256.clone() }, "user"));
                } else if let Some(a) = active_in(meta, harness, Scope::Project, &path).filter(|a| a.surface == "prompt") {
                    selected.push((Pin { id: meta.id.clone(), sha256: a.sha256.clone() }, "project"));
                }
            }
        }
        let mut out = Resolution::default();
        for (pin, origin) in selected {
            let mut resolved = Resolved { id: pin.id.clone(), sha256: pin.sha256.clone(), name: pin.id.clone(), provenance: origin.into(), delivery: "skipped".into(), reason: None };
            let result = (|| -> Result<Option<String>, String> {
                let meta = self.load(&pin.id).map_err(|_| "Skill is missing or its metadata is damaged.")?;
                resolved.name = meta.name.clone();
                if meta.sha256 != pin.sha256 { return Err("A newer version was imported. Select it again to approve that version.".into()); }
                // If both scopes selected different versions, neither silently wins.
                if bot.iter().chain(project.iter()).any(|p| p.id == pin.id && p.sha256 != pin.sha256) { return Err("Bot and project selected different versions. Review their selections.".into()); }
                let files = self.files(&pin.id)?;
                if content_hash(&files) != pin.sha256 { return Err("Library files changed after selection.".into()); }
                let findings: Vec<_> = files.iter().flat_map(|(path, bytes)| super::safety::scan(path, &String::from_utf8_lossy(bytes))).collect();
                if !findings.is_empty() && meta.acknowledged.as_deref() != Some(&meta.sha256) { return Err("Safety findings need explicit acknowledgement.".into()); }
                if let Some(a) = active_in(&meta, harness, Scope::Project, &path).or_else(|| active_in(&meta, harness, Scope::User, &None)) {
                    if a.surface != "prompt" {
                        if a.sha256 == pin.sha256 && Self::status(&meta, Some(a)) == "active" { return Ok(None); }
                        return Err("Existing native activation is outdated or edited. Review its files and activate the selected version explicitly.".into());
                    }
                }
                let main = files.iter().find(|(p, _)| p == super::main_file(meta.kind)).ok_or("Skill content is missing.")?;
                let (_, _, body) = frontmatter(std::str::from_utf8(&main.1).map_err(|_| "Skill is not UTF-8 text.")?);
                let section = format!("\n### {} ({origin})\n{}\n", bounded(&meta.name, 200), bounded(body, ITEM_BYTES));
                if HEADER.len() + out.text.len() + section.len() > CONTEXT_BYTES { return Err("Skill did not fit the 32 KiB context budget.".into()); }
                Ok(Some(section))
            })();
            match result { Ok(Some(section)) => { resolved.delivery = "prompt".into(); out.text.push_str(&section); }, Ok(None) => resolved.delivery = "native".into(), Err(reason) => resolved.reason = Some(reason) }
            out.items.push(resolved);
        }
        if !out.text.is_empty() { out.text.insert_str(0, HEADER); }
        Ok(out)
    }
    fn context_path(&self, session_id: &str) -> std::path::PathBuf {
        self.dir.join(".task-contexts").join(super::sha256_hex(session_id.as_bytes()))
    }
    fn context_hash(provider: &str, resolution: &Resolution) -> Result<String, String> {
        let bytes = serde_json::to_vec(&(provider, &resolution.items, &resolution.text)).map_err(|e| e.to_string())?;
        Ok(super::sha256_hex(&bytes))
    }
    fn previous_context(&self, session_id: &str) -> Result<Option<String>, String> {
        let path = self.context_path(session_id);
        match fs::symlink_metadata(&path) {
            Ok(meta) if !meta.file_type().is_file() || meta.len() > 128 => return Err("The selected-skills context record is invalid.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("Could not read the selected-skills context record: {e}")),
            Ok(_) => {},
        }
        fs::read_to_string(path).map(Some).map_err(|e| e.to_string())
    }
    fn context_changed(&self, session_id: &str, provider: &str, resolution: &Resolution) -> Result<bool, String> {
        let next = Self::context_hash(provider, resolution)?;
        Ok(match self.previous_context(session_id)? {
            Some(previous) => previous != next,
            None => !resolution.items.is_empty(),
        })
    }
    /// Called under task admission's agents/store locks. Invalidate the native context before
    /// recording its new skill identity: a failed filesystem write cannot retain an old resume
    /// id with a new fingerprint. History remains in SQLite and is recovered normally.
    pub(crate) fn synchronize_context(&self, store: &crate::storage::Store, session: &mut crate::storage::Session, resolution: &Resolution) -> Result<(), String> {
        let previous = self.previous_context(&session.id)?;
        if previous.is_none() && resolution.items.is_empty() { return Ok(()); }
        let next = Self::context_hash(&session.provider, resolution)?;
        if previous.as_deref() == Some(next.as_str()) { return Ok(()); }
        if session.resume_id.is_some() {
            let transaction = store.db.unchecked_transaction().map_err(|e| e.to_string())?;
            crate::context_recovery::queue(store, &session.id)?;
            store.db.execute("UPDATE sessions SET resume_id=NULL,context_used_tokens=NULL,context_window=NULL,goal_state=NULL WHERE id=?1", [&session.id]).map_err(|e| e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
            session.resume_id = None; session.context_usage = None; session.goal_state = None;
        }
        let path = self.context_path(&session.id);
        let parent = path.parent().ok_or("Invalid skill context path.")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temporary = parent.join(format!(".{}-{}", super::sha256_hex(session.id.as_bytes()), uuid::Uuid::new_v4()));
        fs::write(&temporary, next).and_then(|_| fs::rename(&temporary, path)).map_err(|e| format!("Could not record selected-skills context; the task was not started: {e}"))
    }

}

/// Resolve under the same store/execution lock as the task being admitted.
pub(crate) fn task(library: &Library, store: &crate::storage::Store, session: &crate::storage::Session, root: &Path) -> Result<Resolution, String> {
    let bot = crate::bots::owner_of(store, &session.id)?.map(|(bot, _)| bot.id)
        .or(crate::bots::list(store)?.into_iter().find(|b| b.project_id == session.project_id).map(|b| b.id));
    library.resolve(&session.provider, root, &session.project_id, bot.as_deref())
}

/// A terminal cannot inject scope text or recover history. A resume id can be emitted before
/// a failed first prompt, so it cannot establish that the selected text was ever delivered.
pub(crate) fn check_console_context(library: &Library, store: &crate::storage::Store, session: &crate::storage::Session, root: &Path) -> Result<(), String> {
    let resolution = task(library, store, session, root)?;
    if !resolution.text.is_empty() {
        return Err("This task delivers selected skills through chat context. Continue in chat, or explicitly activate the skills in the agent's native project files before using the console.".into());
    }
    if library.context_changed(&session.id, &session.provider, &resolution)? {
        return Err("Selected skills changed. Send a normal message in this task first to refresh its context before opening the console.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{tests::Fixture, Kind};
    fn skill(fx: &Fixture, name: &str, body: &str) -> super::super::Item {
        let text = format!("---\nname: {name}\ndescription: {name} instructions\n---\n{body}");
        let path = fx.folder(name, &[("SKILL.md", &text)]); fx.import(Kind::Skill, &path)
    }
    #[test]
    fn selections_deduplicate_preserve_origin_and_do_not_touch_checkout() {
        let fx = Fixture::new(); let root = fx.repo();
        skill(&fx, "Review", "Review the patch."); skill(&fx, "Tests", "Test relevant behavior.");
        fx.library.select(Target::Bot, "bot-a", &["review".into(), "review".into()], false).unwrap();
        fx.library.select(Target::Project, "project-a", &["review".into(), "tests".into()], false).unwrap();
        let result = fx.library.resolve("claude", &root, "project-a", Some("bot-a")).unwrap();
        assert_eq!(result.items.len(), 2); assert_eq!(result.items[0].provenance, "both"); assert_eq!(result.items[1].provenance, "project");
        assert_eq!(result.text.matches("Review the patch.").count(), 1);
        assert!(!root.join(".claude").exists()); assert!(!root.join(".codex").exists());
        assert!(fx.library.resolve("claude", &root, "unrelated", None).unwrap().text.is_empty());
        assert!(fx.library.remove("review").is_err());
    }
    #[test]
    fn imported_updates_need_explicit_scope_reselection_and_drift_is_skipped() {
        let fx = Fixture::new(); let root = fx.repo();
        skill(&fx, "Review", "Original.");
        fx.library.select(Target::Bot, "bot", &["review".into()], false).unwrap();
        skill(&fx, "Review", "New version.");
        let stale = fx.library.resolve("codex", &root, "p", Some("bot")).unwrap();
        assert!(stale.text.is_empty()); assert!(stale.items[0].reason.as_ref().unwrap().contains("newer version"));
        fx.library.select(Target::Bot, "bot", &["review".into()], false).unwrap();
        assert!(fx.library.resolve("codex", &root, "p", Some("bot")).unwrap().text.contains("New version."));
        fs::write(fx.library.dir.join("review/SKILL.md"), "malicious edit").unwrap();
        let drifted = fx.library.resolve("codex", &root, "p", Some("bot")).unwrap();
        assert!(drifted.text.is_empty()); assert!(drifted.items[0].reason.as_ref().unwrap().contains("changed after"));
    }
    #[test]
    fn conflicting_scope_versions_fail_closed() {
        let fx = Fixture::new(); let root = fx.repo();
        skill(&fx, "Review", "Original.");
        fx.library.select(Target::Bot, "bot", &["review".into()], false).unwrap();
        skill(&fx, "Review", "New.");
        fx.library.select(Target::Project, "p", &["review".into()], false).unwrap();
        let result = fx.library.resolve("codex", &root, "p", Some("bot")).unwrap();
        assert!(result.text.is_empty()); assert_eq!(result.items[0].provenance, "both"); assert_eq!(result.items[0].delivery, "skipped");
    }
    #[test]
    fn findings_require_hash_specific_consent_and_prompt_text_is_bounded() {
        let fx = Fixture::new(); let root = fx.repo();
        skill(&fx, "Unsafe", "Use --dangerously-skip-permissions.");
        assert!(fx.library.select(Target::Bot, "bot", &["unsafe".into()], false).is_err());
        fx.library.select(Target::Bot, "bot", &["unsafe".into()], true).unwrap();
        assert!(fx.library.resolve("claude", &root, "p", Some("bot")).unwrap().text.contains("dangerously-skip"));
        let mut ids = Vec::new();
        for i in 0..8 { let name = format!("Long-{i}"); skill(&fx, &name, &"é".repeat(12000)); ids.push(name.to_ascii_lowercase()); }
        fx.library.select(Target::Project, "p", &ids, false).unwrap();
        let result = fx.library.resolve("cline", &root, "p", None).unwrap();
        assert!(result.text.len() <= CONTEXT_BYTES);
        assert!(result.items.iter().any(|i| i.delivery == "skipped"));
        assert!(result.text.contains("Skill text truncated"));
    }
    #[test]
    fn existing_native_files_and_prompt_activations_use_the_same_resolution() {
        let fx = Fixture::new(); let root = fx.repo(); skill(&fx, "Review", "Review code.");
        fx.library.activate("review", "claude", Scope::Project, Some(&root), false).unwrap();
        let native = fx.library.resolve("claude", &root, "p", None).unwrap();
        assert!(native.text.is_empty()); assert_eq!(native.items[0].delivery, "native");
        for harness in ["cline", "kilo"] {
            fx.library.activate("review", harness, Scope::Project, Some(&root), false).unwrap();
            let prompt = fx.library.resolve(harness, &root, "p", None).unwrap();
            assert!(prompt.text.contains("Review code.")); assert_eq!(prompt.items[0].delivery, "prompt");
            assert!(fx.library.resolve(harness, &fx.home, "other", None).unwrap().text.is_empty());
        }
    }
    #[test]
    fn malformed_store_and_unmanaged_hidden_files_fail_closed() {
        let fx = Fixture::new(); let root = fx.repo(); skill(&fx, "Review", "Review code.");
        fs::write(fx.library.dir.join("review/.user-notes"), "keep me").unwrap();
        assert!(fx.library.select(Target::Bot, "bot", &["review".into()], false).is_err());
        assert_eq!(fs::read_to_string(fx.library.dir.join("review/.user-notes")).unwrap(), "keep me");
        fs::write(fx.library.dir.join(".scopes.json"), "not json").unwrap();
        assert!(fx.library.resolve("claude", &root, "p", None).is_err());
    }
    #[test]
    fn unrelated_damaged_items_do_not_block_task_admission() {
        let fx = Fixture::new(); let root = fx.repo();
        skill(&fx, "Selected", "Use only this skill."); skill(&fx, "Unrelated", "Ignore this.");
        fx.library.select(Target::Project, "p", &["selected".into()], false).unwrap();
        fs::write(fx.library.dir.join("unrelated/.notes"), "user file").unwrap();
        let result = fx.library.resolve("codex", &root, "p", None).unwrap();
        assert_eq!(result.items.len(), 1); assert!(result.text.contains("Use only this skill."));
        fs::write(fx.library.dir.join("unrelated.json"), "broken metadata").unwrap();
        assert!(fx.library.resolve("codex", &root, "different", None).unwrap().items.is_empty());
        // A selected damaged item is visible as skipped, never injected.
        fs::write(fx.library.dir.join("selected.json"), "broken metadata").unwrap();
        let result = fx.library.resolve("codex", &root, "p", None).unwrap();
        assert!(result.text.is_empty()); assert_eq!(result.items[0].delivery, "skipped");
    }
    #[test]
    fn explicit_removal_prunes_deleted_targets_but_preserves_existing_selections() {
        let fx = Fixture::new(); let store = crate::storage::Store::open(Path::new(":memory:")).unwrap();
        skill(&fx, "Review", "Review code.");
        let bot = crate::bots::create(&store, &crate::bots::BotInput { name: "Reviewer".into(), provider: "codex".into(), model: "default".into(), mode: "ask".into(), ..Default::default() }, &fx.base.join("bots")).unwrap();
        let root = fx.repo(); let project = store.add_project(&root).unwrap();
        for (target, id) in [(Target::Bot, bot.id.as_str()), (Target::Project, project.id.as_str()), (Target::Bot, "deleted-bot"), (Target::Project, "deleted-project")] {
            fx.library.select(target, id, &["review".into()], false).unwrap();
        }
        // An offline/moved workspace is still a registered project, not a deleted target.
        fs::remove_dir_all(&root).unwrap();
        assert!(fx.library.remove_with_store(&store, "review").unwrap_err().contains("remove its bot/project selections"));
        assert!(fx.library.selection(Target::Bot, "deleted-bot").unwrap().is_empty());
        assert!(fx.library.selection(Target::Project, "deleted-project").unwrap().is_empty());
        assert_eq!(fx.library.selection(Target::Bot, &bot.id).unwrap().len(), 1);
        assert_eq!(fx.library.selection(Target::Project, &project.id).unwrap().len(), 1);
        crate::bots::delete(&store, &bot.id).unwrap(); store.remove_project(&project.id).unwrap();
        assert!(fx.library.remove_with_store(&store, "review").unwrap().is_empty());
        assert!(fx.library.selection(Target::Bot, &bot.id).unwrap().is_empty());
        assert!(fx.library.selection(Target::Project, &project.id).unwrap().is_empty());
    }
    #[test]
    fn removal_never_treats_database_or_cleanup_failure_as_target_absence() {
        let fx = Fixture::new(); let store = crate::storage::Store::open(Path::new(":memory:")).unwrap();
        skill(&fx, "Review", "Review code.");
        fx.library.select(Target::Bot, "deleted-bot", &["review".into()], false).unwrap();
        let scopes = fx.library.dir.join(".scopes.json"); let original = fs::read(&scopes).unwrap();
        store.db.execute("DROP TABLE bots", []).unwrap();
        assert!(fx.library.remove_with_store(&store, "review").unwrap_err().contains("Could not check"));
        assert_eq!(fs::read(&scopes).unwrap(), original);
        assert!(fx.library.dir.join("review/SKILL.md").is_file());
        store.db.execute_batch(crate::bots::SCHEMA).unwrap();
        fs::create_dir(fx.library.dir.join(".scopes.json.tmp")).unwrap();
        assert!(fx.library.remove_with_store(&store, "review").unwrap_err().contains("Could not remove obsolete"));
        assert_eq!(fs::read(&scopes).unwrap(), original);
        assert!(fx.library.dir.join("review/SKILL.md").is_file());
    }

}
