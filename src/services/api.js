import { invoke } from '@tauri-apps/api/core'

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export async function getConfig() {
  return invoke('get_config')
}

export async function saveConfig(config) {
  return invoke('save_config', { config })
}

export async function testConnection() {
  return invoke('test_connection')
}

export async function resetConfig() {
  return invoke('reset_config')
}

export async function applyQualityPreset(preset) {
  return invoke('apply_quality_preset', { preset })
}

export async function getBuiltinProfiles() {
  return invoke('get_builtin_profiles')
}

export async function applyProfile(config, profile) {
  return invoke('apply_profile', { config, profile })
}

export async function getConfigDir() {
  return invoke('get_config_dir')
}

export async function exportConfig(path) {
  return invoke('export_config', { path })
}

export async function importConfig(path) {
  return invoke('import_config', { path })
}

export async function verifyRemotePaths() {
  return invoke('verify_remote_paths')
}

export async function ensureOutputDirs() {
  return invoke('ensure_output_dirs')
}

export async function setupRemoteSldl() {
  return invoke('setup_remote_sldl')
}

export async function getSldlHelp() {
  return invoke('get_sldl_help')
}

// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

export async function startJob(url, jobType = 'auto', associatedAlbumMode = false, opts = {}) {
  return invoke('start_job', {
    req: {
      url,
      jobType,
      associatedAlbumMode,
      outputSubdir: opts.outputSubdir || null,
      listenPort: opts.listenPort || null,
      enqueueOnly: opts.enqueueOnly || false,
    }
  })
}

export async function listJobs(filter = null, sort = null, page = 0, perPage = 50) {
  return invoke('list_jobs', { filter, sort, page, perPage })
}

export async function getJob(jobId) {
  return invoke('get_job', { jobId })
}

export async function refreshJobStatus() {
  return invoke('refresh_job_status')
}

export async function getJobLogs(jobId, lines = 60) {
  return invoke('get_job_logs', { jobId, lines })
}

export async function stopJob(jobId) {
  return invoke('stop_job', { jobId })
}

export async function restartJob(jobId) {
  return invoke('restart_job', { jobId })
}

export async function deleteJob(jobId) {
  return invoke('delete_job', { jobId })
}

export async function clearCompleted() {
  return invoke('clear_completed')
}

export async function ackJob(jobId, acknowledged = true) {
  return invoke('ack_job', { req: { jobId, acknowledged } })
}

export async function batchJobs(jobIds, operation) {
  return invoke('batch_jobs', {
    req: { jobIds, operation }
  })
}

export async function getJobStats() {
  return invoke('get_job_stats')
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

export async function listDownloads(subdir = null, search = null, page = 0, perPage = 100) {
  return invoke('list_downloads', { subdir, search, page, perPage })
}

export async function getDownloadStats() {
  return invoke('get_download_stats')
}

export async function getFileInfo(remotePath) {
  return invoke('get_file_info', { remotePath })
}

export async function deleteFile(remotePath) {
  return invoke('delete_file', { remotePath })
}

export async function revealFile(remotePath) {
  return invoke('reveal_file', { remotePath })
}

export async function playFile(remotePath) {
  return invoke('play_file', { remotePath })
}

export async function getRecentFiles(hours = 24) {
  return invoke('get_recent_files', { hours })
}

export async function searchLibrary(query) {
  return invoke('search_library', { query })
}

export async function listCategories() {
  return invoke('list_categories')
}

export async function listSubdirs(category) {
  return invoke('list_subdirs', { category })
}

// ---------------------------------------------------------------------------
// Associated Album Mode
// ---------------------------------------------------------------------------

export async function discoverAlbumsFromPlaylist(playlistUrl) {
  return invoke('discover_albums_from_playlist', { playlistUrl })
}

export async function startAssociatedAlbumJobs(playlistJobId, selectedAlbums) {
  return invoke('start_associated_album_jobs', {
    req: { playlistJobId, selectedAlbums }
  })
}

export async function getJobFamily(jobId) {
  return invoke('get_job_family', { jobId })
}

export async function estimateFamilyCompletion(jobId) {
  return invoke('estimate_family_completion', { jobId })
}

// ---------------------------------------------------------------------------
// Validation & Health
// ---------------------------------------------------------------------------

export async function validateJob(url) {
  return invoke('validate_job', { url })
}

export async function parseSpotifyUrl(url) {
  return invoke('parse_spotify_url', { url })
}

export async function healthCheck() {
  return invoke('health_check')
}

export async function getRemoteHostInfo() {
  return invoke('get_remote_host_info')
}

// ---------------------------------------------------------------------------
// Quality presets enum (mirrors Rust enum)
// ---------------------------------------------------------------------------

export const QualityPresets = {
  Mp3_320: 'Mp3_320',
  LossyFlexible: 'LossyFlexible',
  FlacPreferred: 'FlacPreferred',
  LosslessOnly: 'LosslessOnly',
}

export const QualityPresetLabels = {
  Mp3_320: 'MP3 320kbps',
  LossyFlexible: 'Lossy (flexible)',
  FlacPreferred: 'FLAC preferred',
  LosslessOnly: 'Lossless only',
}

// ---------------------------------------------------------------------------
// Batch operations enum
// ---------------------------------------------------------------------------

export const BatchOperations = {
  Stop: 'Stop',
  Restart: 'Restart',
  Delete: 'Delete',
  Acknowledge: 'Acknowledge',
}

// ---------------------------------------------------------------------------
// Job sort options
// ---------------------------------------------------------------------------

export const JobSorts = {
  CreatedAt: 'CreatedAt',
  UpdatedAt: 'UpdatedAt',
  Progress: 'Progress',
  Status: 'Status',
}

// ---------------------------------------------------------------------------
// Job status helpers
// ---------------------------------------------------------------------------

export const JobStatus = {
  Pending: 'pending',
  Running: 'running',
  Completed: 'completed',
  Failed: 'failed',
  Stopped: 'stopped',
  Banned: 'banned',
  Queued: 'queued',
}

export const JobStatusLabels = {
  pending: 'Pending',
  running: 'Running',
  completed: 'Completed',
  failed: 'Failed',
  stopped: 'Stopped',
  banned: 'Banned',
  queued: 'Queued',
}

export const JobStatusColors = {
  pending: 'var(--text-muted)',
  running: 'var(--accent)',
  completed: 'var(--success)',
  failed: 'var(--danger)',
  stopped: 'var(--warning)',
  banned: 'var(--danger)',
  queued: 'var(--text-muted)',
}

export function isActive(status) {
  return ['pending', 'running', 'queued'].includes(status)
}

export function isTerminal(status) {
  return ['completed', 'failed', 'stopped'].includes(status)
}
