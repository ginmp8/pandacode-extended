//! Exercises the real execution admission; a catalog/mock UI is not the executor oracle.
use super::*;
#[test]
fn selected_project_skills_enter_the_actual_run_without_changing_saved_user_text() {
    let (state, project_id) = concurrent_task_tests::test_state();
    let folder = std::env::temp_dir().join(format!("pandacode-skill-admission-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap();
    let library = skills::Library::new(folder.join("library"), folder.join("home"));
    library.import(skills::Kind::Skill, vec![("SKILL.md".into(), b"---\nname: Test Guidance\ndescription: Test changes.\n---\nVerify the observed behavior.\n".to_vec())], "Test Guidance", skills::Source { kind: "folder".into(), location: "fixture".into(), git_ref: None, commit: None }).unwrap();
    library.select(skills::scoped::Target::Project, &project_id, &["test-guidance".into()], false).unwrap();
    let task = state.store.lock().unwrap().create_session(&project_id, "codex", "default", "ask").unwrap();
    let (tx, _rx) = unbounded_channel();
    let run = prepare_task_run_in_project(&state, task.id.clone(), Some("Implement my request".into()), vec![], tx, Arc::new(AtomicBool::new(false)), RunAdmission { skills: Some(&library), ..Default::default() }).unwrap();
    assert!(run.config.prompt.contains("Verify the observed behavior."));
    assert_eq!(run.config.mode, "ask");
    assert!(run.config.resume_id.is_none());
    let store = state.store.lock().unwrap();
    let messages = store.messages(&task.id, None).unwrap();
    assert_eq!(messages.last().unwrap().content, "Implement my request");
    assert!(std::fs::read_dir(&folder).unwrap().all(|p| p.unwrap().file_name() != ".codex"));
    drop(store); drop(run); drop(state); std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn malformed_scope_registry_cannot_claim_or_save_a_turn() {
    let (state, project_id) = concurrent_task_tests::test_state();
    let folder = std::env::temp_dir().join(format!("pandacode-skill-admission-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(".scopes.json"), "broken data").unwrap();
    let library = skills::Library::new(folder.clone(), folder.join("home"));
    let task = state.store.lock().unwrap().create_session(&project_id, "codex", "default", "ask").unwrap();
    let (tx, _rx) = unbounded_channel();
    let result = prepare_task_run_in_project(&state, task.id.clone(), Some("Run".into()), vec![], tx, Arc::new(AtomicBool::new(false)), RunAdmission { skills: Some(&library), ..Default::default() });
    assert!(result.is_err());
    assert!(state.agents.lock().unwrap().is_empty());
    assert!(state.store.lock().unwrap().messages(&task.id, None).unwrap().is_empty());
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn changed_or_removed_skills_reset_resume_and_recover_history_before_running() {
    let (state, project_id) = concurrent_task_tests::test_state();
    let folder = std::env::temp_dir().join(format!("pandacode-skill-resume-{}", uuid::Uuid::new_v4()));
    let library = skills::Library::new(folder.clone(), folder.join("home"));
    library.import(skills::Kind::Skill, vec![("SKILL.md".into(), b"---\nname: Review\ndescription: Review code.\n---\nUNIQUE_SKILL_INSTRUCTION\n".to_vec())], "Review", skills::Source { kind: "folder".into(), location: "fixture".into(), git_ref: None, commit: None }).unwrap();
    library.select(skills::scoped::Target::Project, &project_id, &["review".into()], false).unwrap();
    let task = state.store.lock().unwrap().create_session(&project_id, "codex", "default", "ask").unwrap();
    let admit = |text: &str| {
        let (tx, _rx) = unbounded_channel();
        prepare_task_run_in_project(&state, task.id.clone(), Some(text.into()), vec![], tx, Arc::new(AtomicBool::new(false)), RunAdmission { skills: Some(&library), ..Default::default() }).unwrap()
    };
    let first = admit("Initial user request");
    assert!(first.config.prompt.contains("UNIQUE_SKILL_INSTRUCTION"));
    state.agents.lock().unwrap().remove(&task.id);
    {
        let store = state.store.lock().unwrap();
        store.resume(&task.id, "native-context-with-skills").unwrap();
        store.message(&task.id, "assistant", "Previous answer").unwrap();
    }
    let same = admit("Continue with the same skills");
    assert_eq!(same.config.resume_id.as_deref(), Some("native-context-with-skills"));
    state.agents.lock().unwrap().remove(&task.id);
    library.select(skills::scoped::Target::Project, &project_id, &[], false).unwrap();
    let removed = admit("Continue after removing the skill");
    assert!(removed.config.resume_id.is_none()); assert!(removed.context_handoff.is_some());
    assert!(!removed.config.prompt.contains("UNIQUE_SKILL_INSTRUCTION"));
    assert!(removed.config.prompt.contains("Initial user request") && removed.config.prompt.contains("Previous answer"));
    assert!(state.store.lock().unwrap().session(&task.id).unwrap().resume_id.is_none());
    drop(first); drop(same); drop(removed); drop(state); std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn console_requires_chat_for_prompt_skills_even_after_native_session_creation() {
    let (state, project_id) = concurrent_task_tests::test_state();
    let folder = std::env::temp_dir().join(format!("pandacode-skill-console-{}", uuid::Uuid::new_v4()));
    let library = skills::Library::new(folder.clone(), folder.join("home"));
    library.import(skills::Kind::Skill, vec![("SKILL.md".into(), b"---\nname: Review\ndescription: Review code.\n---\nReview changes.\n".to_vec())], "Review", skills::Source { kind: "folder".into(), location: "fixture".into(), git_ref: None, commit: None }).unwrap();
    library.select(skills::scoped::Target::Project, &project_id, &["review".into()], false).unwrap();
    let mut task = state.store.lock().unwrap().create_session(&project_id, "codex", "default", "ask").unwrap();
    let store = state.store.lock().unwrap();
    let root = store.workspace_root(&project_id, Some(&task.id)).unwrap();
    assert!(skills::scoped::check_console_context(&library, &store, &task, &root).is_err());
    let resolution = skills::scoped::task(&library, &store, &task, &root).unwrap();
    library.synchronize_context(&store, &mut task, &resolution).unwrap();
    // Fingerprint and native session creation can both precede a failed prompt write.
    assert!(skills::scoped::check_console_context(&library, &store, &task, &root).is_err());
    store.resume(&task.id, "native-with-skills").unwrap(); task = store.session(&task.id).unwrap();
    assert!(skills::scoped::check_console_context(&library, &store, &task, &root).unwrap_err().contains("Continue in chat"));
    library.select(skills::scoped::Target::Project, &project_id, &[], false).unwrap();
    assert!(skills::scoped::check_console_context(&library, &store, &task, &root).is_err());
    assert_eq!(store.session(&task.id).unwrap().resume_id.as_deref(), Some("native-with-skills"));
    // Healthy native activation remains usable once a chat admission records that context.
    library.activate("review", "codex", skills::Scope::User, None, false).unwrap();
    library.select(skills::scoped::Target::Project, &project_id, &["review".into()], false).unwrap();
    let native = skills::scoped::task(&library, &store, &task, &root).unwrap();
    assert!(native.text.is_empty()); assert_eq!(native.items[0].delivery, "native");
    assert!(skills::scoped::check_console_context(&library, &store, &task, &root).is_err());
    library.synchronize_context(&store, &mut task, &native).unwrap();
    assert!(skills::scoped::check_console_context(&library, &store, &task, &root).is_ok());
    drop(store); drop(state); std::fs::remove_dir_all(folder).unwrap();
}
