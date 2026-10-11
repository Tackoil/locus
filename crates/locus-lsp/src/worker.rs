//! One background validator; superseded queued snapshots are discarded.

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::{
    document::Document,
    state::{Analysis, State},
};

struct Job {
    document: Document,
    analysis: Analysis,
}

pub(crate) struct Worker {
    jobs: Sender<Job>,
    pub(crate) completed: Receiver<Analysis>,
}

impl Default for Worker {
    fn default() -> Self {
        let (jobs, receiver) = unbounded::<Job>();
        let (sender, completed) = unbounded();
        std::thread::spawn(move || {
            for job in receiver {
                if job.analysis.cancelled() {
                    continue;
                }
                let analysis = job.analysis.validate(&job.document);
                if sender.send(analysis).is_err() {
                    break;
                }
            }
        });
        Self { jobs, completed }
    }
}

impl Worker {
    pub(crate) fn submit(&self, state: &State, analysis: Analysis) -> std::io::Result<()> {
        let Some(document) = state.document(analysis.uri()) else {
            return Ok(());
        };
        self.jobs
            .send(Job {
                document: document.clone(),
                analysis,
            })
            .map_err(|error| std::io::Error::other(error.to_string()))
    }
}
