//! In-memory job registry + remote status polling.
//!
//! Jobs are ephemeral. On app restart, the registry is empty.
//! The source of truth for a running job is the remote sldl process
//! and its log file on the remote host.
//!
//! DESIGN: A background Tokio task polls active jobs every N seconds
//! and updates their status by tailing remote logs via SSH.
//! We use a single Mutex<HashMap> for simplicity; contention is low
//! (few jobs, infrequent writes).

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::Local;

use crate::error::{AppError, AppResult};
use crate::models::{Job, JobStatus, ParsedProgress};

#[derive(Default)]
pub struct JobManager {
    jobs: Mutex<HashMap<String, Job>>,
}

impl JobManager {
    // --- CRUD ---

    pub fn insert(&self, job: Job) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.insert(job.id.clone(), job);
    }

    pub fn get(&self, id: &str) -> Option<Job> {
        self.jobs.lock().unwrap().get(id).cloned()
    }

    pub fn get_mut(&self, id: &str) -> Option<std::sync::MutexGuard<'_, HashMap<String, Job>>> {
        // NOTE: This returns the whole map guard. For fine-grained locking
        // you'd use DashMap or similar. Keeping it simple for now.
        let jobs = self.jobs.lock().unwrap();
        if jobs.contains_key(id) {
            Some(jobs)
        } else {
            None
        }
    }

    pub fn remove(&self, id: &str) -> Option<Job> {
        self.jobs.lock().unwrap().remove(id)
    }

    pub fn list(&self) -> Vec<Job> {
        self.jobs.lock().unwrap().values().cloned().collect()
    }

    pub fn list_filtered(&self, status: Option<JobStatus>, search: Option<&str>) -> Vec<Job> {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .filter(|j| {
                let status_ok = status.as_ref().map(|s| j.status == *s).unwrap_or(true);
                let search_ok = search
                    .map(|q| {
                        j.url.to_lowercase().contains(&q.to_lowercase())
                            || j.id.to_lowercase().contains(&q.to_lowercase())
                            || j.job_type.label().to_lowercase().contains(&q.to_lowercase())
                    })
                    .unwrap_or(true);
                status_ok && search_ok
            })
            .cloned()
            .collect()
    }

    pub fn active_count(&self) -> usize {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .filter(|j| j.status.is_active())
            .count()
    }

    pub fn clear_completed(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|_, j| !j.status.is_terminal());
    }

    // --- Status updates ---

    pub fn update_status(&self, id: &str, status: JobStatus) -> AppResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| AppError::JobNotFound(id.into()))?;
        job.status = status;
        job.updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if status.is_terminal() {
            job.completed_at = Some(job.updated_at.clone());
        }
        Ok(())
    }

    pub fn update_progress(&self, id: &str, progress: &str) -> AppResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| AppError::JobNotFound(id.into()))?;
        job.progress = progress.into();
        job.updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        Ok(())
    }

    pub fn apply_parsed_progress(&self, id: &str, parsed: &ParsedProgress) -> AppResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| AppError::JobNotFound(id.into()))?;
        job.downloaded_count = parsed.downloaded;
        job.failed_count = parsed.failed;
        job.not_found_count = parsed.not_found;
        job.percent_complete = parsed.percent;
        if parsed.total > 0 {
            job.total_tracks = Some(parsed.total);
        }
        if let Some(ref action) = parsed.current_action {
            job.progress = format!(
                "{}: {}/{} ({} failed)",
                action,
                parsed.downloaded,
                parsed.total,
                parsed.failed
            );
        }
        job.updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // Auto-detect completion
        if parsed.total > 0 && parsed.downloaded + parsed.failed + parsed.not_found >= parsed.total {
            if parsed.failed == 0 && parsed.not_found == 0 {
                job.status = JobStatus::Completed;
            } else {
                job.status = JobStatus::Completed; // still count as completed with some failures
            }
            job.completed_at = Some(job.updated_at.clone());
        }
        Ok(())
    }

    pub fn set_error(&self, id: &str, error: &str) -> AppResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| AppError::JobNotFound(id.into()))?;
        job.status = JobStatus::Failed;
        job.error_message = Some(error.into());
        job.updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        job.completed_at = Some(job.updated_at.clone());
        Ok(())
    }

    pub fn set_banned(&self, id: &str, retry_after_sec: u64) -> AppResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| AppError::JobNotFound(id.into()))?;
        job.status = JobStatus::Banned;
        job.error_message = Some(format!("Soulseek ban — retry after {}s", retry_after_sec));
        job.updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        Ok(())
    }

    // --- Child jobs (Associated Album Mode) ---

    pub fn add_child(&self, parent_id: &str, child_id: String) -> AppResult<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let parent = jobs
            .get_mut(parent_id)
            .ok_or_else(|| AppError::JobNotFound(parent_id.into()))?;
        parent.child_job_ids.push(child_id);
        Ok(())
    }

    pub fn get_children(&self, parent_id: &str) -> Vec<Job> {
        let jobs = self.jobs.lock().unwrap();
        let parent = match jobs.get(parent_id) {
            Some(p) => p,
            None => return vec![],
        };
        parent
            .child_job_ids
            .iter()
            .filter_map(|cid| jobs.get(cid).cloned())
            .collect()
    }

    /// Aggregate progress across a parent + all children.
    pub fn aggregate_progress(&self, parent_id: &str) -> Option<(usize, usize, usize)> {
        let jobs = self.jobs.lock().unwrap();
        let parent = jobs.get(parent_id)?;
        let mut total = parent.total_tracks.unwrap_or(0);
        let mut downloaded = parent.downloaded_count;
        let mut failed = parent.failed_count;

        for child_id in &parent.child_job_ids {
            if let Some(child) = jobs.get(child_id) {
                total += child.total_tracks.unwrap_or(0);
                downloaded += child.downloaded_count;
                failed += child.failed_count;
            }
        }
        Some((downloaded, failed, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mgr = JobManager::default();
        let job = Job {
            id: "test-1".into(),
            ..Default::default()
        };
        mgr.insert(job.clone());
        assert_eq!(mgr.get("test-1").unwrap().id, "test-1");
    }

    #[test]
    fn list_filtered() {
        let mgr = JobManager::default();
        mgr.insert(Job {
            id: "a".into(),
            url: "https://spotify.com/playlist/abc".into(),
            ..Default::default()
        });
        mgr.insert(Job {
            id: "b".into(),
            url: "https://spotify.com/album/def".into(),
            ..Default::default()
        });
        let results = mgr.list_filtered(None, Some("album"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "b");
    }
}
