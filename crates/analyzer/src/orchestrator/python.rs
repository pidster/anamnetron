//! Python orchestrator — delegates to DescriptorOrchestrator.

use super::descriptor::DescriptorOrchestrator;
use crate::languages::python::PythonAnalyzer;

/// Create a Python orchestrator using the descriptor + parser pattern.
#[must_use]
pub fn orchestrator() -> DescriptorOrchestrator {
    DescriptorOrchestrator::new(PythonAnalyzer::descriptor(), PythonAnalyzer::parser())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::LanguageOrchestrator;

    #[test]
    fn python_orchestrator_language_id() {
        let orch = orchestrator();
        assert_eq!(orch.language_id(), "python");
    }
}
