import {
  startJob, refreshJobStatus, getJobLogs, stopJob,
  testConnection, getConfig, getJobStats, validateJob,
  parseSpotifyUrl, discoverAlbumsFromPlaylist,
  startAssociatedAlbumJobs, getJobFamily,
  isActive, JobStatus, JobStatusLabels, JobStatusColors,
} from './services/api.js'

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const state = {
  config: null,
  jobs: [],
  selectedJobId: null,
  connectionOk: false,
  connectionMessage: 'Checking...',
  pollInterval: null,
  autoRefresh: true,
  isDiscoveringAlbums: false,
  albumDiscoveryResult: null,
  pendingAlbumModeJobId: null,
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

export async function init() {
  renderApp()
  await loadConfig()
  await refreshConnection()
  await refreshJobs()
  startPolling()
}

// ---------------------------------------------------------------------------
// Core actions
// ---------------------------------------------------------------------------

async function loadConfig() {
  try {
    state.config = await getConfig()
  } catch (e) {
    console.error('Failed to load config:', e)
  }
}

async function refreshConnection() {
  try {
    const status = await testConnection()
    state.connectionOk = status.ok
    state.connectionMessage = status.message
  } catch (e) {
    state.connectionOk = false
    state.connectionMessage = 'Disconnected'
  }
  updateConnectionBadge()
}

async function refreshJobs() {
  try {
    state.jobs = await refreshJobStatus()
    renderJobs()
    updateStats()
  } catch (e) {
    console.error('Refresh failed:', e)
  }
}

async function onDownload() {
  const input = document.getElementById('url-input')
  const url = input.value.trim()
  if (!url) return alert('Enter a Spotify URL')

  const typeEl = document.getElementById('job-type')
  const assocEl = document.getElementById('associated-album-mode')

  const jobType = typeEl.value
  const assoc = assocEl.checked

  // Validate
  try {
    const validation = await validateJob(url)
    if (validation.warnings.length > 0) {
      const ok = confirm(validation.warnings.join('\n') + '\n\nProceed anyway?')
      if (!ok) return
    }
  } catch (e) {
    console.warn('Validation failed:', e)
  }

  try {
    const result = await startJob(url, jobType, assoc)
    input.value = ''
    state.selectedJobId = result.jobId

    if (assoc) {
      // Enter album discovery flow
      state.pendingAlbumModeJobId = result.jobId
      await discoverAlbums(result.jobId, url)
    }

    await refreshJobs()
    await viewLogs(result.jobId)
  } catch (e) {
    alert('Failed to start job: ' + e)
  }
}

async function discoverAlbums(jobId, url) {
  state.isDiscoveringAlbums = true
  renderAlbumDiscovery()
  try {
    const result = await discoverAlbumsFromPlaylist(url)
    state.albumDiscoveryResult = result
    state.isDiscoveringAlbums = false
    renderAlbumDiscovery()
  } catch (e) {
    state.isDiscoveringAlbums = false
    state.albumDiscoveryResult = null
    alert('Album discovery failed: ' + e)
  }
}

async function onStartAlbumDownloads() {
  if (!state.albumDiscoveryResult || !state.pendingAlbumModeJobId) return

  const checkboxes = document.querySelectorAll('.album-discovery-item input:checked')
  const selected = Array.from(checkboxes).map(cb => ({
    artist: cb.dataset.artist,
    album: cb.dataset.album,
    priority: parseInt(cb.dataset.priority) || 0,
  }))

  if (selected.length === 0) {
    alert('Select at least one album')
    return
  }

  try {
    await startAssociatedAlbumJobs(state.pendingAlbumModeJobId, selected)
    state.albumDiscoveryResult = null
    state.pendingAlbumModeJobId = null
    renderAlbumDiscovery()
    await refreshJobs()
  } catch (e) {
    alert('Failed to start album jobs: ' + e)
  }
}

async function onViewLogs(jobId) {
  state.selectedJobId = jobId
  renderJobs()
  try {
    const logs = await getJobLogs(jobId, 60)
    renderLogs(logs)
  } catch (e) {
    renderLogs('Error: ' + e)
  }
}

async function onStopJob(jobId) {
  if (!confirm('Stop this job?')) return
  try {
    await stopJob(jobId)
    await refreshJobs()
  } catch (e) {
    alert('Failed to stop: ' + e)
  }
}

// ---------------------------------------------------------------------------
// Polling
// ---------------------------------------------------------------------------

function startPolling() {
  if (state.pollInterval) clearInterval(state.pollInterval)
  state.pollInterval = setInterval(() => {
    if (state.autoRefresh && state.jobs.some(j => isActive(j.status))) {
      refreshJobs()
      if (state.selectedJobId) {
        viewLogs(state.selectedJobId)
      }
    }
  }, 3000)
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

function renderApp() {
  const app = document.getElementById('app')
  app.innerHTML = `
    <header>
      <h1>Sldl Remote</h1>
      <div class="actions">
        <span id="conn-badge" class="conn-status"><span class="dot"></span>Checking...</span>
        <button class="btn" id="btn-settings">Settings</button>
      </div>
    </header>
    <main>
      <section class="input-section">
        <input type="text" id="url-input" placeholder="Paste Spotify URL (playlist, album, artist, or track)..." />
        <div class="options-row">
          <select id="job-type">
            <option value="auto">Auto-detect</option>
            <option value="playlist">Playlist</option>
            <option value="album">Album</option>
            <option value="artist">Artist (Discography)</option>
            <option value="track">Track</option>
          </select>
          <label class="toggle-label" title="Download the full album for each track in this playlist">
            <input type="checkbox" id="associated-album-mode" />
            <span>Associated Album Mode</span>
          </label>
          <button class="btn primary" id="btn-download">Download</button>
        </div>
      </section>
      <section class="log-section">
        <h2>
          <span>Live Log</span>
          <span id="log-meta"></span>
        </h2>
        <pre id="log-viewer"><div class="empty">Select a job to view logs</div></pre>
      </section>
      <aside class="status-panel">
        <h2>Active Jobs <span id="job-count"></span></h2>
        <div id="jobs-list"><div class="empty">No jobs yet</div></div>
      </aside>
    </main>
    <div id="modal-container"></div>
    <div id="album-discovery-overlay"></div>
  `

  document.getElementById('btn-download').addEventListener('click', onDownload)
  document.getElementById('btn-settings').addEventListener('click', openSettings)
  document.getElementById('url-input').addEventListener('keydown', e => {
    if (e.key === 'Enter') onDownload()
  })
}

function updateConnectionBadge() {
  const el = document.getElementById('conn-badge')
  if (!el) return
  el.className = state.connectionOk ? 'conn-status ok' : 'conn-status err'
  el.innerHTML = `<span class="dot"></span>${state.connectionMessage}`
}

function renderJobs() {
  const container = document.getElementById('jobs-list')
  const countEl = document.getElementById('job-count')
  if (!container) return

  const active = state.jobs.filter(j => isActive(j.status)).length
  if (countEl) countEl.textContent = active > 0 ? `(${active})` : ''

  if (state.jobs.length === 0) {
    container.innerHTML = '<div class="empty">No jobs yet</div>'
    return
  }

  container.innerHTML = state.jobs.map(job => {
    const isSelected = job.id === state.selectedJobId
    const statusColor = JobStatusColors[job.status] || 'var(--text-muted)'
    const progressText = job.percentComplete != null
      ? `${job.percentComplete.toFixed(1)}% — ${job.progress}`
      : job.progress

    return `
      <div class="job-card ${isSelected ? 'active' : ''}" data-id="${escapeHtml(job.id)}">
        <div class="job-header">
          <span class="job-type">${job.jobType}</span>
          <span class="job-status" style="background:${statusColor};color:white">${JobStatusLabels[job.status] || job.status}</span>
        </div>
        <div class="job-url">${escapeHtml(job.url)}</div>
        <div class="job-progress">${escapeHtml(progressText)}</div>
        ${job.associatedAlbumMode ? '<div class="album-badge">Associated Album Mode</div>' : ''}
        <div class="job-actions">
          <button class="btn small" data-action="logs" data-id="${escapeHtml(job.id)}">Logs</button>
          ${isActive(job.status) ? `
            <button class="btn small danger" data-action="stop" data-id="${escapeHtml(job.id)}">Stop</button>
          ` : ''}
        </div>
      </div>
    `
  }).join('')

  container.querySelectorAll('[data-action="logs"]').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation()
      onViewLogs(btn.dataset.id)
    })
  })
  container.querySelectorAll('[data-action="stop"]').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation()
      onStopJob(btn.dataset.id)
    })
  })
  container.querySelectorAll('.job-card').forEach(card => {
    card.addEventListener('click', () => {
      onViewLogs(card.dataset.id)
    })
  })
}

function renderLogs(text) {
  const viewer = document.getElementById('log-viewer')
  const meta = document.getElementById('log-meta')
  if (!viewer) return

  const job = state.jobs.find(j => j.id === state.selectedJobId)
  if (meta && job) {
    meta.textContent = `${job.jobType} • ${job.progress}`
  }

  viewer.textContent = text || 'No logs available'
  viewer.scrollTop = viewer.scrollHeight
}

function updateStats() {
  // Placeholder for stats bar update
}

function renderAlbumDiscovery() {
  const overlay = document.getElementById('album-discovery-overlay')
  if (!overlay) return

  if (!state.isDiscoveringAlbums && !state.albumDiscoveryResult) {
    overlay.innerHTML = ''
    overlay.style.display = 'none'
    return
  }

  overlay.style.display = 'flex'

  if (state.isDiscoveringAlbums) {
    overlay.innerHTML = `
      <div class="discovery-modal">
        <h2>Discovering Albums...</h2>
        <p>Scanning playlist for unique albums. This may take a moment.</p>
        <div class="spinner"></div>
      </div>
    `
    return
  }

  const result = state.albumDiscoveryResult
  if (!result) return

  overlay.innerHTML = `
    <div class="discovery-modal">
      <h2>Associated Album Mode</h2>
      <p>Found ${result.uniqueAlbums.length} unique albums across ${result.trackCount} tracks.</p>
      <div class="album-list">
        ${result.uniqueAlbums.map((album, i) => `
          <label class="album-discovery-item">
            <input type="checkbox" checked
              data-artist="${escapeHtml(album.artist)}"
              data-album="${escapeHtml(album.album)}"
              data-priority="${result.trackCount - i}"
            />
            <span>${escapeHtml(album.artist)} — ${escapeHtml(album.album)}</span>
          </label>
        `).join('')}
      </div>
      <div class="discovery-actions">
        <button class="btn" id="btn-cancel-albums">Cancel</button>
        <button class="btn primary" id="btn-start-albums">Download ${result.uniqueAlbums.length} Albums</button>
      </div>
    </div>
  `

  document.getElementById('btn-cancel-albums').addEventListener('click', () => {
    state.albumDiscoveryResult = null
    state.pendingAlbumModeJobId = null
    renderAlbumDiscovery()
  })
  document.getElementById('btn-start-albums').addEventListener('click', onStartAlbumDownloads)
}

// ---------------------------------------------------------------------------
// Settings Modal
// ---------------------------------------------------------------------------

function openSettings() {
  const container = document.getElementById('modal-container')
  const c = state.config || {}

  container.innerHTML = `
    <div class="modal-overlay" id="settings-overlay">
      <div class="modal">
        <div class="modal-header">
          <h2>Settings</h2>
          <button class="modal-close" id="btn-close-settings">&times;</button>
        </div>
        <div class="modal-body" id="settings-body">
          <!-- Tabs -->
          <div class="tabs">
            <button class="tab active" data-tab="remote">Remote Host</button>
            <button class="tab" data-tab="accounts">Accounts</button>
            <button class="tab" data-tab="quality">Quality</button>
            <button class="tab" data-tab="advanced">Advanced</button>
          </div>
          <div class="tab-content active" data-tab="remote">
            ${renderRemoteTab(c)}
          </div>
          <div class="tab-content" data-tab="accounts">
            ${renderAccountsTab(c)}
          </div>
          <div class="tab-content" data-tab="quality">
            ${renderQualityTab(c)}
          </div>
          <div class="tab-content" data-tab="advanced">
            ${renderAdvancedTab(c)}
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn" id="btn-test-conn">Test Connection</button>
          <button class="btn primary" id="btn-save-settings">Save</button>
        </div>
      </div>
    </div>
  `

  document.getElementById('btn-close-settings').addEventListener('click', closeSettings)
  document.getElementById('settings-overlay').addEventListener('click', e => {
    if (e.target.id === 'settings-overlay') closeSettings()
  })
  document.getElementById('btn-save-settings').addEventListener('click', saveSettings)
  document.getElementById('btn-test-conn').addEventListener('click', testConnFromSettings)

  // Tab switching
  container.querySelectorAll('.tab').forEach(tab => {
    tab.addEventListener('click', () => {
      container.querySelectorAll('.tab').forEach(t => t.classList.remove('active'))
      container.querySelectorAll('.tab-content').forEach(t => t.classList.remove('active'))
      tab.classList.add('active')
      container.querySelector(`.tab-content[data-tab="${tab.dataset.tab}"]`).classList.add('active')
    })
  })
}

function closeSettings() {
  document.getElementById('modal-container').innerHTML = ''
}

async function testConnFromSettings() {
  const btn = document.getElementById('btn-test-conn')
  btn.textContent = 'Testing...'
  btn.disabled = true
  try {
    await saveSettings(false)
    await refreshConnection()
    btn.textContent = state.connectionOk ? 'Connected!' : 'Failed'
  } catch (e) {
    btn.textContent = 'Error'
  }
  setTimeout(() => { btn.textContent = 'Test Connection'; btn.disabled = false }, 2000)
}

async function saveSettings(close = true) {
  const newConfig = {
    remoteHost: document.getElementById('cfg-host').value,
    remoteUser: document.getElementById('cfg-user').value,
    sshPort: parseInt(document.getElementById('cfg-port').value) || 22,
    sshKeyPath: document.getElementById('cfg-key').value,
    sldlPath: document.getElementById('cfg-sldl').value,
    outputPath: document.getElementById('cfg-output').value,
    soulseekUsername: document.getElementById('cfg-sl-user').value,
    soulseekPassword: document.getElementById('cfg-sl-pass').value,
    spotifyId: document.getElementById('cfg-sp-id').value,
    spotifySecret: document.getElementById('cfg-sp-secret').value,
    prefFormat: document.getElementById('cfg-format').value,
    prefMinBitrate: parseInt(document.getElementById('cfg-bitrate').value) || 320,
    searchesPerTime: parseInt(document.getElementById('cfg-spt').value) || 20,
    searchesRenewTime: parseInt(document.getElementById('cfg-renew').value) || 300,
    fastSearch: document.getElementById('cfg-fast').checked,
    nameFormat: document.getElementById('cfg-name').value,
    maxRetries: parseInt(document.getElementById('cfg-retries').value) || 10,
    skipNotFound: document.getElementById('cfg-skip').checked,
    concurrentJobs: parseInt(document.getElementById('cfg-concurrent').value) || 20,
    listenPortBase: parseInt(document.getElementById('cfg-port-base').value) || 49900,
  }

  try {
    await invoke('save_config', { config: newConfig })
    state.config = newConfig
    if (close) closeSettings()
  } catch (e) {
    alert('Save failed: ' + e)
  }
}

function renderRemoteTab(c) {
  return `
    <div class="form-row">
      <div class="form-group"><label>Host</label><input type="text" id="cfg-host" value="${esc(c.remoteHost)}" /></div>
      <div class="form-group"><label>User</label><input type="text" id="cfg-user" value="${esc(c.remoteUser)}" /></div>
    </div>
    <div class="form-row">
      <div class="form-group"><label>SSH Port</label><input type="number" id="cfg-port" value="${c.sshPort || 22}" /></div>
      <div class="form-group"><label>SSH Key Path</label><input type="text" id="cfg-key" value="${esc(c.sshKeyPath)}" placeholder="~/.ssh/id_rsa" /></div>
    </div>
    <div class="form-row">
      <div class="form-group"><label>sldl Path (remote)</label><input type="text" id="cfg-sldl" value="${esc(c.sldlPath)}" /></div>
      <div class="form-group"><label>Output Path (remote)</label><input type="text" id="cfg-output" value="${esc(c.outputPath)}" /></div>
    </div>
  `
}

function renderAccountsTab(c) {
  return `
    <div class="form-row">
      <div class="form-group"><label>Soulseek Username</label><input type="text" id="cfg-sl-user" value="${esc(c.soulseekUsername)}" /></div>
      <div class="form-group"><label>Soulseek Password</label><input type="password" id="cfg-sl-pass" value="${esc(c.soulseekPassword)}" /></div>
    </div>
    <div class="form-row">
      <div class="form-group"><label>Spotify Client ID</label><input type="text" id="cfg-sp-id" value="${esc(c.spotifyId)}" /></div>
      <div class="form-group"><label>Spotify Client Secret</label><input type="password" id="cfg-sp-secret" value="${esc(c.spotifySecret)}" /></div>
    </div>
  `
}

function renderQualityTab(c) {
  return `
    <div class="form-row">
      <div class="form-group">
        <label>Preferred Format</label>
        <select id="cfg-format">
          <option value="mp3" ${c.prefFormat === 'mp3' ? 'selected' : ''}>MP3</option>
          <option value="flac" ${c.prefFormat === 'flac' ? 'selected' : ''}>FLAC</option>
          <option value="mp3,flac" ${c.prefFormat === 'mp3,flac' ? 'selected' : ''}>MP3 or FLAC</option>
          <option value="flac,wav" ${c.prefFormat === 'flac,wav' ? 'selected' : ''}>FLAC or WAV</option>
        </select>
      </div>
      <div class="form-group"><label>Min Bitrate</label><input type="number" id="cfg-bitrate" value="${c.prefMinBitrate || 320}" /></div>
    </div>
    <div class="form-row">
      <div class="form-group"><label>Searches / Time</label><input type="number" id="cfg-spt" value="${c.searchesPerTime || 20}" /></div>
      <div class="form-group"><label>Renew Time (sec)</label><input type="number" id="cfg-renew" value="${c.searchesRenewTime || 300}" /></div>
    </div>
    <div class="form-row">
      <div class="form-group"><label>Name Format</label><input type="text" id="cfg-name" value="${esc(c.nameFormat)}" /></div>
    </div>
  `
}

function renderAdvancedTab(c) {
  return `
    <div class="form-row">
      <div class="form-group"><label>Max Retries</label><input type="number" id="cfg-retries" value="${c.maxRetries || 10}" /></div>
      <div class="form-group"><label>Concurrent Jobs</label><input type="number" id="cfg-concurrent" value="${c.concurrentJobs || 20}" /></div>
    </div>
    <div class="form-row">
      <div class="form-group"><label>Listen Port Base</label><input type="number" id="cfg-port-base" value="${c.listenPortBase || 49900}" /></div>
      <div class="form-group">
        <label class="toggle-label">
          <input type="checkbox" id="cfg-fast" ${c.fastSearch ? 'checked' : ''} />
          Fast Search
        </label>
      </div>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="toggle-label">
          <input type="checkbox" id="cfg-skip" ${c.skipNotFound !== false ? 'checked' : ''} />
          Skip Not Found
        </label>
      </div>
    </div>
  `
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

function escapeHtml(text) {
  if (!text) return ''
  const div = document.createElement('div')
  div.textContent = text
  return div.innerHTML
}

const esc = escapeHtml

// Expose for inline handlers
window.viewLogs = onViewLogs
window.stopJob = onStopJob
