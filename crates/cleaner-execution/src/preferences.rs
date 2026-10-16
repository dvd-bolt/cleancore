use crate::{
    native::{BoundDelete, TrustedCandidate},
    ActionResult, ObjectAction,
};
use cleaner_core::cleanup_preferences::CleanupPreferences;
use std::sync::{atomic::AtomicBool, Arc, RwLock};
pub type SharedPreferences = Arc<RwLock<Option<CleanupPreferences>>>;
pub struct PreferenceDelete {
    pub preferences: SharedPreferences,
}
impl ObjectAction for PreferenceDelete {
    fn available(&self) -> bool {
        cfg!(windows) && self.preferences.read().is_ok_and(|p| p.is_some())
    }
    fn irreversible(&self) -> bool {
        true
    }
    fn apply(&self, candidate: &TrustedCandidate) -> ActionResult {
        self.apply_with_cancel(candidate, &AtomicBool::new(false))
    }
    fn apply_with_cancel(&self, candidate: &TrustedCandidate, cancel: &AtomicBool) -> ActionResult {
        let Ok(guard) = self.preferences.read() else {
            return ActionResult::Skipped("PREFERENCES_UNAVAILABLE".into());
        };
        let Some(preferences) = guard.as_ref() else {
            return ActionResult::Skipped("PREFERENCES_UNAVAILABLE".into());
        };
        if preferences.excludes(&candidate.display_path()) {
            return ActionResult::Skipped("USER_EXCLUDED".into());
        }
        if candidate.copy_guard.is_some()
            && preferences.protects_duplicate(&candidate.display_path())
        {
            return ActionResult::Skipped("PROTECTED_ORIGINAL".into());
        }
        BoundDelete.apply_with_cancel(candidate, cancel)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn changing_exclusions_blocks_an_already_captured_candidate() {
        let root =
            std::env::temp_dir().join(format!("cleancore_exclusion_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("keep.txt"), b"fixture").unwrap();
        let mut candidate =
            crate::native::capture_candidate(&root, std::path::Path::new("keep.txt")).unwrap();
        candidate.minimum_age_ms = 0;
        let shared = Arc::new(RwLock::new(Some(CleanupPreferences::default())));
        let action = PreferenceDelete {
            preferences: shared.clone(),
        };
        shared
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .excluded_paths
            .push(root.to_string_lossy().into_owned());
        assert!(
            matches!(action.apply(&candidate), ActionResult::Skipped(code) if code == "USER_EXCLUDED")
        );
        assert!(root.join("keep.txt").exists());
        *shared.write().unwrap() = None;
        assert!(!action.available());
        assert!(
            matches!(action.apply(&candidate), ActionResult::Skipped(code) if code == "PREFERENCES_UNAVAILABLE")
        );
        *shared.write().unwrap() = Some(CleanupPreferences::default());
        assert!(matches!(action.apply(&candidate), ActionResult::Deleted));
        std::fs::remove_dir_all(root).unwrap();
    }
}
