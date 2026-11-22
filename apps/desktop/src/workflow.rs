use std::path::Path;
use std::sync::Arc;

use super::files::{ScenarioFile, check_destination, output_path};
use super::{output::StagedOutput, results, scenario};

pub(super) struct OpenedScenario {
    pub source: Arc<ScenarioFile>,
    pub rail: scenario::RailScenario,
}

impl OpenedScenario {
    /// Validates the file once and retains the tick-zero inputs for repeatable runs.
    pub fn open(path: &Path) -> Result<Self, String> {
        let (source, rail) = ScenarioFile::open(path)?;
        Ok(Self {
            source: Arc::new(source),
            rail,
        })
    }

    /// Returns a saveable recording only after execution, analysis and size validation succeed.
    pub fn run(&self) -> Result<Recording, String> {
        let mut rail = self.rail.clone();
        let (data, json) = results::record(
            rail.world,
            &mut rail.vehicle,
            &mut rail.clock,
            &mut rail.passengers,
            rail.max_ticks,
        )
        .map_err(|detail| format!("{}: {detail}", self.source.path.display()))?;

        Ok(Recording {
            data,
            source: self.source.clone(),
            json,
        })
    }
}

pub(super) struct Recording {
    pub data: results::RailResult,
    pub source: Arc<ScenarioFile>,
    json: String,
}

impl Recording {
    /// Replaces the destination with the validated run after staging its complete contents.
    /// Both inputs keep their chosen names and file identities protected across edits and renames.
    pub fn save(&self, path: &Path, selected: &ScenarioFile) -> Result<(), String> {
        let inputs = [self.source.as_ref(), selected];
        let destination = output_path(path, &inputs)?;

        StagedOutput::new(&destination, &self.json, None)?.replace(&destination, |staged| {
            check_destination(staged, path, &destination, &inputs)
        })
    }
}
