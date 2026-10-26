use super::*;
use std::sync::mpsc;

struct Fixture {
    parent: PathBuf,
    accounts: Vec<String>,
}

impl Fixture {
    fn empty() -> Self {
        let parent =
            std::env::temp_dir().join(format!("pandacode-jev-startup-{}", uuid::Uuid::new_v4()));
        private_dir(&parent.join("jev")).unwrap();
        Self {
            parent,
            accounts: Vec::new(),
        }
    }

    fn path(&self) -> PathBuf {
        self.parent.join("jev/settings.json")
    }

    fn settings() -> Settings {
        Settings {
            enabled: true,
            decisions_enabled: true,
            decision_provider: "openrouter".into(),
            decision_model: "test/saved-model".into(),
            python_path: std::env::temp_dir()
                .join("saved-python")
                .to_string_lossy()
                .into(),
            ..Settings::default()
        }
    }

    fn all_keys() -> Self {
        let mut fixture = Self::empty();
        let saved = persist_saved(
            &fixture.path(),
            &Saved {
                settings: Self::settings(),
                typesafe_key: "fixture-typesafe".into(),
                text_key: "fixture-text".into(),
                openrouter_key: "fixture-router".into(),
                ..Saved::default()
            },
        )
        .unwrap();
        let reference = saved.credential_ref.unwrap();
        fixture.accounts = ["typesafe", "text", "openrouter"]
            .map(|name| format!("{}.{name}", reference.id))
            .into();
        fixture
    }

    fn legacy() -> Self {
        let fixture = Self::empty();
        let saved = json!({
            "settings": Self::settings(),
            "typesafeKey": "fixture-typesafe",
            "textKey": "fixture-text",
            "openrouterKey": "fixture-router",
        });
        private_write(&fixture.path(), &serde_json::to_vec(&saved).unwrap()).unwrap();
        fixture
    }

    fn referenced() -> Self {
        let parent =
            std::env::temp_dir().join(format!("pandacode-jev-startup-{}", uuid::Uuid::new_v4()));
        let dir = parent.join("jev");
        private_dir(&dir).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let account = format!("{id}.typesafe");
        crate::secret_store::set(SECRET_SERVICE, &account, "fixture-saved-key").unwrap();
        let saved = Saved {
            credential_ref: Some(CredentialRef {
                id,
                typesafe: true,
                text: false,
                openrouter: false,
            }),
            ..Saved::default()
        };
        private_write(
            &dir.join("settings.json"),
            &serde_json::to_vec(&saved).unwrap(),
        )
        .unwrap();
        Self {
            parent,
            accounts: vec![account],
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(saved) = read_json::<Saved>(&self.path(), MAX_LINE) {
            if let Some(reference) = saved.credential_ref {
                for name in ["typesafe", "text", "openrouter"] {
                    crate::secret_store::remove(
                        SECRET_SERVICE,
                        &format!("{}.{name}", reference.id),
                    );
                }
            }
        }
        for account in &self.accounts {
            crate::secret_store::remove(SECRET_SERVICE, account);
        }
        let _ = std::fs::remove_dir_all(&self.parent);
    }
}

struct Release(Option<mpsc::Sender<()>>);
impl Release {
    fn release(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        self.release();
    }
}

#[test]
fn setup_returns_while_the_actual_vault_read_is_pending() {
    let fixture = Fixture::referenced();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (entered, observed) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let mut release = Release(Some(release));
    let (returned, result) = mpsc::channel();
    let initializer = instance.clone();
    let thread = std::thread::spawn(move || {
        // This is the same initializer that setup calls. Only its platform vault
        // boundary is held; settings parsing, validation and publication are real.
        initializer.initialize(move |path| {
            load_saved_with(
                path,
                |account| {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                    crate::secret_store::get(SECRET_SERVICE, account)
                },
                persist_saved,
            )
        });
        returned.send(()).unwrap();
    });
    observed
        .recv_timeout(Duration::from_secs(5))
        .expect("the real credential reader must be entered");
    let returned_while_held = result.recv_timeout(Duration::from_secs(1)).is_ok();
    let runtime_usable = instance.runtime.try_lock().is_ok();
    release.release();
    thread.join().unwrap();
    tauri::async_runtime::block_on(instance.ready()).unwrap();
    assert!(
        returned_while_held,
        "setup must return before the held OS vault read completes"
    );
    assert!(
        runtime_usable,
        "vault hydration must not hold the Runtime mutex"
    );
    assert_eq!(
        instance.runtime.lock().unwrap().saved.typesafe_key,
        "fixture-saved-key"
    );
}

// All holds are inside the real settings loader. The release guard also wakes
// the blocking worker if an assertion fails, so it cannot stay blocked.
fn hold_read(
    instance: &Arc<Instance>,
) -> (
    mpsc::Receiver<()>,
    Release,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let (entered, observed) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = reads.clone();
    instance.initialize(move |path| {
        let mut wait = Some(wait);
        load_saved_with(
            path,
            |account| {
                counter.fetch_add(1, Ordering::SeqCst);
                if let Some(wait) = wait.take() {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                }
                crate::secret_store::get(SECRET_SERVICE, account)
            },
            persist_saved,
        )
    });
    (observed, Release(Some(release)), reads)
}

fn expect_read(observed: &mpsc::Receiver<()>) {
    observed
        .recv_timeout(Duration::from_secs(5))
        .expect("the real credential reader must be entered");
}

fn assert_saved(configuration: &Configuration) {
    assert!(configuration.settings.enabled && configuration.settings.decisions_enabled);
    assert_eq!(configuration.settings.decision_provider, "openrouter");
    assert_eq!(configuration.settings.decision_model, "test/saved-model");
    assert!(
        configuration.typesafe_key_saved
            && configuration.text_key_saved
            && configuration.openrouter_key_saved
    );
    let serialized = serde_json::to_string(configuration).unwrap();
    for secret in ["fixture-typesafe", "fixture-text", "fixture-router"] {
        assert!(!serialized.contains(secret));
    }
}

#[tokio::test]
async fn configuration_waiters_do_not_expose_defaults_or_restart_cancelled_hydration() {
    let fixture = Fixture::all_keys();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (observed, mut release, reads) = hold_read(&instance);
    expect_read(&observed);
    let mut cancelled = Box::pin(instance.configuration());
    let mut first = Box::pin(instance.configuration());
    let mut second = Box::pin(instance.configuration());
    assert!(futures_util::poll!(cancelled.as_mut()).is_pending());
    assert!(futures_util::poll!(first.as_mut()).is_pending());
    assert!(futures_util::poll!(second.as_mut()).is_pending());
    drop(cancelled);
    {
        let state = instance
            .runtime
            .try_lock()
            .expect("catalog and shutdown must remain able to lock Runtime");
        assert!(!state.saved.settings.enabled && !state.saved.settings.decisions_enabled);
        assert!(state.active.is_empty() && !state.installing);
        assert!(
            state.saved.typesafe_key.is_empty()
                && state.saved.text_key.is_empty()
                && state.saved.openrouter_key.is_empty()
        );
    }
    release.release();
    let (first, second) = tokio::join!(first, second);
    assert_saved(&first.unwrap());
    assert_saved(&second.unwrap());
    assert_saved(&instance.configuration().await.unwrap());
    assert_eq!(
        reads.load(Ordering::SeqCst),
        3,
        "one hydration reads each referenced key once"
    );
}

#[tokio::test]
async fn completion_without_waiters_is_retained_and_empty_settings_never_touch_the_vault() {
    let fixture = Fixture::empty();
    let instance = Instance::pending(&fixture.parent).unwrap();
    instance.initialize(|path| {
        load_saved_with(
            path,
            |_| panic!("fresh settings must not read the vault"),
            |_, _| panic!("fresh settings must not migrate credentials"),
        )
    });
    // Observe the sender only: no readiness receiver exists when completion is
    // published. Sender::send would lose that result; send_replace retains it.
    tokio::time::timeout(Duration::from_secs(5), async {
        while matches!(*instance.initialized.borrow(), Initialization::Pending) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let config = tokio::time::timeout(Duration::from_secs(1), instance.configuration())
        .await
        .unwrap()
        .unwrap();
    assert!(!config.settings.enabled && !config.settings.decisions_enabled);
    assert!(!config.typesafe_key_saved && !config.text_key_saved && !config.openrouter_key_saved);
    assert!(instance.lock().unwrap().setup_warning.is_none());
    assert!(!fixture.path().exists());
}

#[tokio::test]
async fn save_waits_for_hydration_then_merges_keys_without_a_late_overwrite() {
    let fixture = Fixture::all_keys();
    let original = std::fs::read(fixture.path()).unwrap();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (observed, mut release, reads) = hold_read(&instance);
    expect_read(&observed);
    let settings = Settings {
        decision_model: "test/requested-model".into(),
        enabled: false,
        ..Fixture::settings()
    };
    let mut save = Box::pin(instance.save(settings, None, Some(String::new()), None));
    assert!(futures_util::poll!(save.as_mut()).is_pending());
    assert_eq!(
        std::fs::read(fixture.path()).unwrap(),
        original,
        "pending Save cannot commit a generation"
    );
    for (account, key) in
        fixture
            .accounts
            .iter()
            .zip(["fixture-typesafe", "fixture-text", "fixture-router"])
    {
        assert_eq!(
            crate::secret_store::get(SECRET_SERVICE, account).as_deref(),
            Some(key)
        );
    }
    release.release();
    let result = save.await.unwrap();
    assert!(!result.settings.enabled && result.settings.decisions_enabled);
    assert_eq!(result.settings.decision_model, "test/requested-model");
    assert!(result.typesafe_key_saved && !result.text_key_saved && result.openrouter_key_saved);
    let (restarted, warning) = load_saved(&fixture.path());
    assert!(warning.is_none());
    assert_eq!(restarted.typesafe_key, "fixture-typesafe");
    assert!(restarted.text_key.is_empty());
    assert_eq!(restarted.openrouter_key, "fixture-router");
    assert_eq!(restarted.settings.decision_model, "test/requested-model");
    assert_eq!(
        instance
            .configuration()
            .await
            .unwrap()
            .settings
            .decision_model,
        "test/requested-model"
    );
    assert_eq!(reads.load(Ordering::SeqCst), 3);
    assert!(fixture
        .accounts
        .iter()
        .all(|account| crate::secret_store::get(SECRET_SERVICE, account).is_none()));
}

#[tokio::test]
async fn failed_read_publishes_no_partial_keys_and_save_requires_recovery() {
    let fixture = Fixture::all_keys();
    crate::secret_store::remove(SECRET_SERVICE, &fixture.accounts[2]);
    let original = std::fs::read(fixture.path()).unwrap();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (observed, mut release, reads) = hold_read(&instance);
    expect_read(&observed);
    let mut save = Box::pin(instance.save(Settings::default(), Some(String::new()), None, None));
    assert!(futures_util::poll!(save.as_mut()).is_pending());
    release.release();
    let error = save
        .await
        .err()
        .expect("Save must not erase an inaccessible recovery generation");
    assert!(error.contains("still unavailable"));
    let config = instance.configuration().await.unwrap();
    assert!(!config.settings.enabled && !config.settings.decisions_enabled);
    assert!(!config.typesafe_key_saved && !config.text_key_saved && !config.openrouter_key_saved);
    assert!(instance.lock().unwrap().setup_warning.is_some());
    assert_eq!(std::fs::read(fixture.path()).unwrap(), original);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        3,
        "the final failed read must not publish earlier keys"
    );
    crate::secret_store::set(SECRET_SERVICE, &fixture.accounts[2], "fixture-router").unwrap();
    let config = instance
        .save(Fixture::settings(), None, None, None)
        .await
        .unwrap();
    assert_saved(&config);
    assert!(instance.lock().unwrap().setup_warning.is_none());
    let (saved, warning) = load_saved(&fixture.path());
    assert!(warning.is_none());
    assert_eq!(saved.typesafe_key, "fixture-typesafe");
    assert_eq!(saved.text_key, "fixture-text");
    assert_eq!(saved.openrouter_key, "fixture-router");
}

#[tokio::test]
async fn legacy_migration_is_detached_and_save_waits_for_its_commit() {
    let fixture = Fixture::legacy();
    let original = std::fs::read(fixture.path()).unwrap();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (entered, observed) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let mut release = Release(Some(release));
    instance.initialize(move |path| {
        load_saved_with(
            path,
            |_| panic!("legacy migration does not read credential references"),
            |path, saved| {
                entered.send(()).unwrap();
                wait.recv().unwrap();
                persist_saved(path, saved)
            },
        )
    });
    expect_read(&observed);
    assert!(
        instance.runtime.try_lock().is_ok(),
        "migration must not hold Runtime"
    );
    let mut save = Box::pin(instance.save(
        Settings {
            decision_model: "test/after-migration".into(),
            ..Fixture::settings()
        },
        None,
        None,
        None,
    ));
    assert!(futures_util::poll!(save.as_mut()).is_pending());
    assert_eq!(std::fs::read(fixture.path()).unwrap(), original);
    release.release();
    let config = save.await.unwrap();
    assert_eq!(config.settings.decision_model, "test/after-migration");
    assert!(config.typesafe_key_saved && config.text_key_saved && config.openrouter_key_saved);
    let bytes = std::fs::read_to_string(fixture.path()).unwrap();
    for key in ["fixture-typesafe", "fixture-text", "fixture-router"] {
        assert!(!bytes.contains(key));
    }
    let (saved, warning) = load_saved(&fixture.path());
    assert!(warning.is_none());
    assert_eq!(saved.settings.decision_model, "test/after-migration");
    assert_eq!(saved.openrouter_key, "fixture-router");
}

#[tokio::test]
async fn failed_legacy_migration_preserves_recovery_and_settles_readiness() {
    let fixture = Fixture::legacy();
    let original = std::fs::read(fixture.path()).unwrap();
    let instance = Instance::pending(&fixture.parent).unwrap();
    instance.initialize(|path| {
        load_saved_with(
            path,
            |_| None,
            |path, saved| {
                persist_saved_with(
                    saved,
                    |_, _| Err("vault is locked".into()),
                    |_| {},
                    |bytes| private_write(path, bytes),
                )
            },
        )
    });
    let config = tokio::time::timeout(Duration::from_secs(5), instance.configuration())
        .await
        .unwrap()
        .unwrap();
    assert!(!config.settings.enabled && !config.settings.decisions_enabled);
    assert!(!config.typesafe_key_saved && !config.text_key_saved && !config.openrouter_key_saved);
    assert_eq!(std::fs::read(fixture.path()).unwrap(), original);
    assert!(instance.lock().unwrap().setup_warning.is_some());
    // Once the vault works again, the existing explicit Save recovery path
    // migrates the original file before applying the requested settings.
    assert_saved(
        &instance
            .save(Fixture::settings(), None, None, None)
            .await
            .unwrap(),
    );
    assert!(!std::fs::read_to_string(fixture.path())
        .unwrap()
        .contains("fixture-typesafe"));
}

#[tokio::test]
async fn blocking_worker_failure_settles_disabled_recovery_instead_of_waiting_forever() {
    let fixture = Fixture::all_keys();
    let original = std::fs::read(fixture.path()).unwrap();
    let instance = Instance::pending(&fixture.parent).unwrap();
    instance.initialize(|_| panic!("controlled initialization failure"));
    let config = tokio::time::timeout(Duration::from_secs(5), instance.configuration())
        .await
        .unwrap()
        .unwrap();
    assert!(!config.settings.enabled && !config.settings.decisions_enabled);
    assert!(!config.typesafe_key_saved && !config.text_key_saved && !config.openrouter_key_saved);
    assert_eq!(std::fs::read(fixture.path()).unwrap(), original);
    assert_eq!(
        instance.lock().unwrap().setup_warning.as_deref(),
        Some(SETUP_WARNING)
    );
}

#[tokio::test]
async fn doctor_snapshot_and_installer_admission_wait_for_the_saved_python_path() {
    let fixture = Fixture::all_keys();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (state, _) = crate::concurrent_task_tests::test_state();
    let (observed, mut release, _) = hold_read(&instance);
    expect_read(&observed);
    let mut doctor = Box::pin(instance.snapshot());
    let mut install = Box::pin(instance.begin_install(&state));
    assert!(futures_util::poll!(doctor.as_mut()).is_pending());
    assert!(futures_util::poll!(install.as_mut()).is_pending());
    assert!(!instance.lock().unwrap().installing);
    assert!(state.agents.try_lock().is_ok() && state.store.try_lock().is_ok());
    release.release();
    let (_, saved, warning) = doctor.await.unwrap();
    let (dir, settings) = install.await.unwrap();
    assert_eq!(saved.settings.python_path, Fixture::settings().python_path);
    assert_eq!(settings.python_path, Fixture::settings().python_path);
    assert!(warning.is_none());
    assert_eq!(dir, fixture.parent.join("jev"));
    assert!(instance.lock().unwrap().installing);
    assert!(
        !venv_python(&dir).exists(),
        "preparation must not launch an installer"
    );
}

#[tokio::test]
async fn pending_installer_rechecks_shutdown_before_reserving() {
    let fixture = Fixture::all_keys();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (state, _) = crate::concurrent_task_tests::test_state();
    let (observed, mut release, _) = hold_read(&instance);
    expect_read(&observed);
    let mut install = Box::pin(instance.begin_install(&state));
    assert!(futures_util::poll!(install.as_mut()).is_pending());
    state.shutdown.store(1, Ordering::SeqCst);
    release.release();
    assert!(install.await.err().unwrap().contains("closing"));
    assert!(!instance.lock().unwrap().installing);
}

fn browser_report(sid: &str) -> Report {
    Report::starting(
        sid,
        &Request {
            url: "http://127.0.0.1:3000".into(),
            goal: "Check local fixture".into(),
            max_steps: 20,
            timeout_seconds: 120,
            viewport: "desktop".into(),
            expected_text: None,
            expected_title: None,
            expected_url: None,
        },
    )
}

#[tokio::test]
async fn browser_admission_waits_for_real_credentials_before_reserving() {
    let fixture = Fixture::all_keys();
    private_write(
        &venv_python(&fixture.parent.join("jev")),
        b"admission-only fixture",
    )
    .unwrap();
    let instance = Instance::pending(&fixture.parent).unwrap();
    let (state, project) = crate::concurrent_task_tests::test_state();
    let sid = state
        .store
        .lock()
        .unwrap()
        .create_session(&project, "codex", "default", "ask")
        .unwrap()
        .id;
    let report = browser_report(&sid);
    let (observed, mut release, _) = hold_read(&instance);
    expect_read(&observed);
    let mut begin =
        Box::pin(instance.begin_test(&state, &report, Arc::new(AtomicBool::new(false)), None));
    assert!(futures_util::poll!(begin.as_mut()).is_pending());
    assert!(instance.lock().unwrap().active.is_empty());
    assert!(state.agents.try_lock().is_ok() && state.store.try_lock().is_ok());
    release.release();
    let (_, saved) = begin.await.unwrap();
    assert_eq!(saved.typesafe_key, "fixture-typesafe");
    assert_eq!(saved.text_key, "fixture-text");
    assert!(instance.lock().unwrap().active.contains_key(&report.id));
}

#[tokio::test]
async fn pending_browser_admission_rechecks_cancellation_shutdown_and_task_deletion() {
    for change in ["cancel", "shutdown", "delete"] {
        let fixture = Fixture::all_keys();
        private_write(
            &venv_python(&fixture.parent.join("jev")),
            b"admission-only fixture",
        )
        .unwrap();
        let instance = Instance::pending(&fixture.parent).unwrap();
        let (state, project) = crate::concurrent_task_tests::test_state();
        let sid = state
            .store
            .lock()
            .unwrap()
            .create_session(&project, "codex", "default", "ask")
            .unwrap()
            .id;
        let report = browser_report(&sid);
        let parent_stopped = AtomicBool::new(false);
        let (observed, mut release, _) = hold_read(&instance);
        expect_read(&observed);
        let mut begin = Box::pin(instance.begin_test(
            &state,
            &report,
            Arc::new(AtomicBool::new(false)),
            Some(&parent_stopped),
        ));
        assert!(futures_util::poll!(begin.as_mut()).is_pending());
        match change {
            "cancel" => parent_stopped.store(true, Ordering::SeqCst),
            "shutdown" => state.shutdown.store(1, Ordering::SeqCst),
            "delete" => {
                crate::task_deletion::delete_state(&state, &sid).unwrap();
            }
            _ => unreachable!(),
        }
        release.release();
        let error = begin
            .await
            .err()
            .expect("stale browser admission must be rejected");
        if change == "cancel" {
            assert!(error.contains("stopped"));
        }
        if change == "shutdown" {
            assert!(error.contains("closing"));
        }
        assert!(instance.lock().unwrap().active.is_empty());
    }
}
