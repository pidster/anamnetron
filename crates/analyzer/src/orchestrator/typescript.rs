//! TypeScript/Svelte orchestrator — delegates to DescriptorOrchestrator.

use super::descriptor::DescriptorOrchestrator;
use crate::languages::typescript::TypeScriptAnalyzer;

/// Create a TypeScript orchestrator using the descriptor + parser pattern.
#[must_use]
pub fn orchestrator() -> DescriptorOrchestrator {
    DescriptorOrchestrator::new(
        TypeScriptAnalyzer::descriptor(),
        TypeScriptAnalyzer::parser(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::LanguageOrchestrator;

    #[test]
    fn typescript_orchestrator_language_id() {
        let orch = orchestrator();
        assert_eq!(orch.language_id(), "typescript");
    }

    #[test]
    fn typescript_orchestrator_discovers_packages() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let orch = orchestrator();
        let units = orch.discover(&project_root);
        assert!(!units.is_empty(), "should discover at least 1 TS package");
        assert!(units.iter().all(|u| u.language == "typescript"));
    }

    #[test]
    fn typescript_orchestrator_emits_structural_items() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let orch = orchestrator();
        let units = orch.discover(&project_root);
        assert!(!units.is_empty());
        let items = orch.emit_structural_items(&units[0]);
        assert!(
            !items.is_empty(),
            "should emit structural module items for TS package"
        );
    }
}
